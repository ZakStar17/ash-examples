use std::mem::{self, MaybeUninit};

use ash::vk;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::{
  dpi::PhysicalSize,
  event_loop::EventLoopWindowTarget,
  window::{Window, WindowBuilder},
};

use crate::{
  destroy,
  render::{
    command_pools::{
      GraphicsCommandBufferPool, TemporaryGraphicsCommandPool, TransferCommandBufferPool,
    },
    data::GPUData,
    device_destroyable::{DeviceManuallyDestroyed, ManuallyDestroyed},
    errors::{InitializationError, OutOfMemoryError},
    initialization::{
      self,
      device::{create_logical_device, PhysicalDevice, Queues},
    },
    pipelines::{self, GraphicsPipeline},
    render_pass::{
      create_framebuffer, create_framebuffers_from_swapchain_images, create_render_pass,
    },
  },
  utility::OnErr,
  INITIAL_WINDOW_HEIGHT, INITIAL_WINDOW_WIDTH, WINDOW_TITLE,
};

use super::{
  descriptor_sets::DescriptorPool,
  initialization::Surface,
  pipelines::PipelineCreationError,
  render_object::RenderPosition,
  swapchain::{SwapchainCreationError, Swapchains},
  RenderInit, FRAMES_IN_FLIGHT,
};

#[derive(Debug)]
pub enum SwapchainRecreationError {
  OutOfMemory(OutOfMemoryError),
  SwapchainError(SwapchainCreationError),
  PipelineCreationError(PipelineCreationError),
}

impl From<OutOfMemoryError> for SwapchainRecreationError {
  fn from(value: OutOfMemoryError) -> Self {
    SwapchainRecreationError::OutOfMemory(value)
  }
}

impl From<SwapchainCreationError> for SwapchainRecreationError {
  fn from(value: SwapchainCreationError) -> Self {
    SwapchainRecreationError::SwapchainError(value)
  }
}

impl From<PipelineCreationError> for SwapchainRecreationError {
  fn from(value: PipelineCreationError) -> Self {
    SwapchainRecreationError::PipelineCreationError(value)
  }
}

pub struct Renderer {
  _entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: initialization::DebugUtils,
  physical_device: PhysicalDevice,
  pub device: ash::Device,
  pub queues: Queues,

  window: Window,
  surface: Surface,

  pub swapchains: Swapchains,
  render_pass: vk::RenderPass,
  framebuffers: Box<[vk::Framebuffer]>,
  old_framebuffers: (bool, Box<[vk::Framebuffer]>),

  descriptor_pool: DescriptorPool,
  pipeline_cache: vk::PipelineCache,
  pipeline: GraphicsPipeline,
  pub command_pools: [GraphicsCommandBufferPool; FRAMES_IN_FLIGHT],

  gpu_data: GPUData,
}

const DESTROYABLE_COUNT: usize = 69; // todo
struct Destructor {
  objs: [MaybeUninit<*const dyn DeviceManuallyDestroyed>; DESTROYABLE_COUNT],
  len: usize,
}

impl Destructor {
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
      .build(target)?;

    let mut destructor = Destructor::new();

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
      create_logical_device(&instance, &physical_device).on_err(|_| destroy_instance())?;
    destructor.push(&device);

    let swapchains = Swapchains::new(
      &instance,
      &physical_device,
      &device,
      &surface,
      window.inner_size(),
    )?;
    destructor.push(&swapchains);

    let render_pass = create_render_pass(&device, swapchains.get_format())
      .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&render_pass);

    let framebuffers = create_framebuffers_from_swapchain_images(&device, &swapchains, render_pass)
      .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&framebuffers);
    let old_framebuffers = (0..swapchains.get_image_views().len())
      .map(|_| vk::Framebuffer::null())
      .collect();

    let mut descriptor_pool =
      DescriptorPool::new(&device).on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&descriptor_pool);

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

    let command_pool_1 =
      GraphicsCommandBufferPool::create(&device, &physical_device.queue_families)
        .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&command_pool_1);
    let command_pool_2 =
      GraphicsCommandBufferPool::create(&device, &physical_device.queue_families)
        .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&command_pool_2);
    let command_pools: [GraphicsCommandBufferPool; FRAMES_IN_FLIGHT] =
      [command_pool_1, command_pool_2];

    let data = {
      let mut temp_transfer_pool =
        TransferCommandBufferPool::create(&device, &physical_device.queue_families)
          .on_err(|_| unsafe { destructor.fire(&device) })?;
      let mut temp_graphics_pool =
        TemporaryGraphicsCommandPool::create(&device, &physical_device.queue_families).on_err(
          |_| unsafe {
            temp_transfer_pool.destroy_self(&device);
            destructor.fire(&device);
          },
        )?;

      let data = GPUData::new(
        &device,
        &physical_device,
        &queues,
        &mut descriptor_pool,
        &mut temp_transfer_pool,
        &mut temp_graphics_pool,
      )
      .on_err(|_| unsafe {
        temp_transfer_pool.destroy_self(&device);
        temp_graphics_pool.destroy_self(&device);
        destructor.fire(&device);
      })?;
      log::debug!("GPU data addresses: {:#?}", data);

      unsafe {
        temp_transfer_pool.destroy_self(&device);
        temp_graphics_pool.destroy_self(&device);
      }

      data
    };

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
      gpu_data: data,
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
      &self.gpu_data,
      position,
    )?;
    Ok(())
  }

  pub unsafe fn recreate_swapchain(&mut self) -> Result<(), SwapchainRecreationError> {
    // most of this function is just cleanup in case of an error

    // it is possible to use more than two frames in flight, but it would require having more than one old swapchain and pipeline
    assert!(FRAMES_IN_FLIGHT == 2);

    // this function shouldn't be called if old objects haven't been destroyed
    assert!(!self.old_framebuffers.0);

    // old swapchain becomes retired
    let changes = self.swapchains.recreate(
      &self.physical_device,
      &self.device,
      &self.surface,
      self.window.inner_size(),
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
    } else {
      if !changes.extent {
        log::warn!("Recreating swapchain without any extent or format change");
      }
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

      self.gpu_data.destroy_self(&self.device);

      self.pipeline.destroy_self(&self.device);
      self.pipeline_cache.destroy_self(&self.device);
      self.descriptor_pool.destroy_self(&self.device);

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
