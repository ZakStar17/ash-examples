use std::mem::{self, MaybeUninit};

use ash::vk;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::{
  dpi::PhysicalSize,
  event_loop::EventLoopWindowTarget,
  window::{Window, WindowBuilder},
};

use crate::{
  ferris::Ferris,
  render::{
    format_conversions::{self, KNOWN_FORMATS},
    initialization::device,
  },
  utility::OnErr,
  INITIAL_WINDOW_HEIGHT, INITIAL_WINDOW_WIDTH, WINDOW_TITLE,
};

use super::{
  command_pools::GraphicsCommandBufferPool,
  descriptor_sets::DescriptorPool,
  device_destroyable::{
    destroy, fill_destroyable_array_with_expression, DeviceManuallyDestroyed, ManuallyDestroyed,
  },
  errors::{InitializationError, OutOfMemoryError, SwapchainRecreationError},
  gpu_data::GPUData,
  initialization::{
    self,
    device::{Device, PhysicalDevice, Queues},
    Surface,
  },
  pipelines::{self, GraphicsPipeline},
  render_object::RenderPosition,
  render_pass::{
    create_framebuffer, create_framebuffers_from_swapchain_images, create_render_pass,
  },
  swapchain::{SwapchainCreationError, Swapchains},
  RenderInit, FRAMES_IN_FLIGHT, SWAPCHAIN_IMAGE_USAGES,
};

const TEXTURE_PATH: &str = "./ferris.png";

fn read_texture_bytes_as_rgba8() -> Result<(u32, u32, Vec<u8>), image::ImageError> {
  let img = image::ImageReader::open(TEXTURE_PATH)?
    .decode()?
    .into_rgba8();
  let width = img.width();
  let height = img.height();

  let bytes = img.into_raw();
  assert!(bytes.len() == width as usize * height as usize * 4);
  Ok((width, height, bytes))
}

pub struct Renderer {
  _entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: initialization::DebugUtils,
  physical_device: PhysicalDevice,
  pub device: Device,
  pub queues: Queues,

  pub window: Window,
  surface: Surface,

  pub swapchains: Swapchains,
  render_pass: vk::RenderPass,
  framebuffers: Box<[vk::Framebuffer]>,
  old_framebuffers: (bool, Box<[vk::Framebuffer]>),

  pipeline_cache: vk::PipelineCache,
  pipeline: GraphicsPipeline,
  pub command_pools: [GraphicsCommandBufferPool; FRAMES_IN_FLIGHT],

  data: GPUData,
  descriptor_pool: DescriptorPool,
}

struct Destructor<const N: usize> {
  objs: [MaybeUninit<*const dyn DeviceManuallyDestroyed>; N],
  len: usize,
}

impl<const N: usize> Destructor<N> {
  pub fn new() -> Self {
    Self {
      objs: unsafe { MaybeUninit::uninit().assume_init() },
      len: 0,
    }
  }

  pub fn push(&mut self, ptr: *const dyn DeviceManuallyDestroyed) {
    self.len += 1;
    self.objs[self.len] = MaybeUninit::new(ptr);
  }

  pub unsafe fn fire(&self, device: &ash::Device) {
    for i in (0..self.len).rev() {
      self.objs[i]
        .assume_init()
        .as_ref()
        .unwrap()
        .destroy_self(device);
    }
  }
}

impl Renderer {
  pub fn initialize(
    pre_window: RenderInit,
    target: &EventLoopWindowTarget<()>,
  ) -> Result<Self, InitializationError> {
    // having an error during window creation triggers pre_window drop
    let window = WindowBuilder::new()
      .with_title(WINDOW_TITLE)
      .with_inner_size(PhysicalSize {
        width: INITIAL_WINDOW_WIDTH,
        height: INITIAL_WINDOW_HEIGHT,
      })
      .with_min_inner_size(PhysicalSize {
        width: Ferris::WIDTH,
        height: Ferris::HEIGHT,
      })
      // .with_resizable(false)
      .build(target)?;

    let mut destructor: Destructor<14> = Destructor::new();

    #[cfg(feature = "vl")]
    let (entry, instance, debug_utils) = pre_window.deconstruct();
    #[cfg(not(feature = "vl"))]
    let (entry, instance) = pre_window.deconstruct();

    let destroy_instance = || unsafe {
      #[cfg(feature = "vl")]
      destroy!(&debug_utils);
      destroy!(&instance);
    };
    destructor.push(&instance);
    #[cfg(feature = "vl")]
    destructor.push(&debug_utils);

    let surface = Surface::new(
      &entry,
      &instance,
      target.display_handle()?,
      window.window_handle()?,
    )
    .on_err(|_| destroy_instance())?;
    destructor.push(&surface);

    let physical_device = match unsafe { PhysicalDevice::select(&instance, &surface) }
      .on_err(|_| destroy_instance())?
    {
      Some(device) => device,
      None => {
        destroy_instance();
        return Err(InitializationError::NoCompatibleDevices);
      }
    };

    let (device, queues) =
      Device::create(&instance, &physical_device).on_err(|_| destroy_instance())?;
    destructor.push(&device);

    let swapchains = Swapchains::new(
      &instance,
      &physical_device,
      &device,
      &surface,
      window.inner_size(),
      SWAPCHAIN_IMAGE_USAGES,
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&swapchains);

    let swapchain_format = swapchains.get_format();
    let texture_format = if KNOWN_FORMATS.contains(&swapchain_format) {
      swapchain_format
    } else {
      KNOWN_FORMATS
        .into_iter()
        .find(|&f| device::format_is_supported(&instance, *physical_device, f))
        .unwrap()
    };

    let (width, height, mut texture_data) = read_texture_bytes_as_rgba8()?;
    let texture_extent = vk::Extent2D { width, height };
    format_conversions::convert_rgba_data_to_format(&mut texture_data, texture_format);
    log::info!("Creating texture with the format {:?}", texture_format);

    let (gpu_data, gpu_data_pending_initialization) = GPUData::new(
      &device,
      &physical_device,
      texture_extent,
      texture_format,
      texture_data,
      &queues,
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&gpu_data);
    destructor.push(&gpu_data_pending_initialization);

    let render_pass = create_render_pass(&device, swapchains.get_format())
      .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&render_pass);

    let framebuffers = create_framebuffers_from_swapchain_images(&device, &swapchains, render_pass)
      .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&framebuffers);
    let old_framebuffers = (0..swapchains.get_image_views().len())
      .map(|_| vk::Framebuffer::null())
      .collect();

    log::info!("Creating pipeline cache");
    let (pipeline_cache, created_from_file) =
      pipelines::create_pipeline_cache(&device, &physical_device)
        .on_err(|_| unsafe { destructor.fire(&device) })?;
    if created_from_file {
      log::info!("Cache successfully created from an existing cache file");
    } else {
      log::info!("Cache initialized as empty");
    }
    destructor.push(&pipeline_cache);

    let descriptor_pool = DescriptorPool::new(&device, gpu_data.texture_view)
      .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&descriptor_pool);

    log::debug!("Creating pipeline");
    let graphics_pipeline = GraphicsPipeline::new(
      &device,
      pipeline_cache,
      render_pass,
      &descriptor_pool,
      swapchains.get_extent(),
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&graphics_pipeline);

    let command_pools = fill_destroyable_array_with_expression!(
      &device,
      GraphicsCommandBufferPool::create(&device, &physical_device.queue_families),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(command_pools.as_ptr());

    unsafe {
      gpu_data_pending_initialization
        .wait_and_self_destroy(&device)
        .on_err(|_| destructor.fire(&device))?;
    }

    Ok(Self {
      window,
      surface,
      _entry: entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
      physical_device,
      device,
      queues,
      command_pools,
      data: gpu_data,
      render_pass,
      pipeline: graphics_pipeline,
      pipeline_cache,
      swapchains,
      descriptor_pool,
      framebuffers,
      old_framebuffers: (false, old_framebuffers),
    })
  }

  pub unsafe fn record_graphics(
    &mut self,
    frame_i: usize,
    image_i: usize,
    position: &RenderPosition,
  ) -> Result<(), OutOfMemoryError> {
    self.command_pools[frame_i].reset(&self.device)?;
    self.command_pools[frame_i].record_main(
      &self.device,
      self.render_pass,
      self.swapchains.get_extent(),
      self.framebuffers[image_i],
      &self.pipeline,
      &self.descriptor_pool,
      &self.data,
      position,
    )?;
    Ok(())
  }

  pub unsafe fn recreate_swapchain(&mut self) -> Result<(), SwapchainRecreationError> {
    // most of this function is just cleanup in case of an error

    // it is possible to use more than two frames in flight, but it would require having more than one old swapchain and pipeline
    #[allow(clippy::assertions_on_constants)]
    {
      assert!(FRAMES_IN_FLIGHT == 2);
    }

    // this function shouldn't be called if old objects haven't been destroyed
    assert!(!self.old_framebuffers.0);

    // old swapchain becomes retired
    let changes = self.swapchains.recreate(
      &self.physical_device,
      &self.device,
      &self.surface,
      self.window.inner_size(),
      SWAPCHAIN_IMAGE_USAGES,
    )?;

    let mut new_render_pass = None;

    if changes.format {
      log::info!("Changing swapchain format");

      // this shouldn't happen regularly, so its okay to stop all rendering so that the render pass can be recreated
      self
        .device
        .device_wait_idle()
        .on_err(|_| self.swapchains.revert_recreate(&self.device))
        .map_err(|vkerr| match vkerr {
          vk::Result::ERROR_OUT_OF_DEVICE_MEMORY | vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
            SwapchainCreationError::OutOfMemory(vkerr.into())
          }
          vk::Result::ERROR_DEVICE_LOST => SwapchainCreationError::DeviceIsLost,
          _ => panic!(),
        })?;

      // recreate all objects that depend on image format (but not on extent)
      new_render_pass = Some(
        create_render_pass(&self.device, self.swapchains.get_format())
          .on_err(|_| self.swapchains.revert_recreate(&self.device))?,
      );
    } else if !changes.extent {
      log::warn!("Recreating swapchain without any extent or format change");
    }

    assert!(self.swapchains.get_image_views().len() == self.framebuffers.len());

    // recreate framebuffers
    for (i, &view) in self.swapchains.get_image_views().iter().enumerate() {
      self.old_framebuffers.1[i] = match create_framebuffer(
        &self.device,
        self.render_pass,
        view,
        self.swapchains.get_extent(),
      ) {
        Ok(v) => v,
        Err(err) => unsafe {
          for framebuffer in self.old_framebuffers.1[0..i].iter() {
            framebuffer.destroy_self(&self.device);
          }

          if let Some(render_pass) = new_render_pass {
            render_pass.destroy_self(&self.device);
          }
          self.swapchains.revert_recreate(&self.device);

          return Err(err.into());
        },
      };
    }

    if changes.extent || changes.format {
      match self.pipeline.recreate(
        &self.device,
        self.pipeline_cache,
        self.render_pass,
        self.swapchains.get_extent(),
      ) {
        Ok(v) => v,
        Err(err) => unsafe {
          for framebuffer in self.old_framebuffers.1.iter() {
            framebuffer.destroy_self(&self.device);
          }
          if let Some(render_pass) = new_render_pass {
            render_pass.destroy_self(&self.device);
          }
          self.swapchains.revert_recreate(&self.device);

          return Err(err.into());
        },
      }
    }

    mem::swap(&mut self.framebuffers, &mut self.old_framebuffers.1);
    self.old_framebuffers.0 = true;
    if let Some(new) = new_render_pass {
      self.render_pass.destroy_self(&self.device);
      self.render_pass = new;
    }

    Ok(())
  }

  // destroy old objects that resulted of a swapchain recreation
  // this should only be called when they stop being in use
  pub unsafe fn destroy_old(&mut self) {
    self.pipeline.destroy_old(&self.device);

    if self.old_framebuffers.0 {
      for fb in self.old_framebuffers.1.iter() {
        fb.destroy_self(&self.device);
      }
      self.old_framebuffers.0 = false;
    }

    self.swapchains.destroy_old(&self.device);
  }
}

impl Drop for Renderer {
  fn drop(&mut self) {
    log::debug!("Destroying renderer objects...");
    unsafe {
      // wait until all operations have finished and the device is safe to destroy
      self
        .device
        .device_wait_idle()
        .expect("Failed to wait for the device to become idle during drop");

      self.destroy_old();

      log::info!("Saving pipeline cache");
      if let Err(err) =
        pipelines::save_pipeline_cache(&self.device, &self.physical_device, self.pipeline_cache)
      {
        log::error!("Failed to save pipeline cache: {:?}", err);
      }

      self.command_pools.destroy_self(&self.device);

      self.pipeline.destroy_self(&self.device);
      self.pipeline_cache.destroy_self(&self.device);
      self.descriptor_pool.destroy_self(&self.device);

      self.data.destroy_self(&self.device);

      self.framebuffers.destroy_self(&self.device);
      self.render_pass.destroy_self(&self.device);
      self.swapchains.destroy_self(&self.device);

      ManuallyDestroyed::destroy_self(&self.surface);
      ManuallyDestroyed::destroy_self(&self.device);

      #[cfg(feature = "vl")]
      {
        ManuallyDestroyed::destroy_self(&self.debug_utils);
      }
      ManuallyDestroyed::destroy_self(&self.instance);
    }
  }
}
