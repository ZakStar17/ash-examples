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
    command_pools::{
      GraphicsCommandBufferPool, TemporaryGraphicsCommandPool, TransferCommandBufferPool,
    },
    data::constant::create_and_populate_constant_data,
    device_destroyable::{destroy, DeviceManuallyDestroyed, ManuallyDestroyed},
    errors::{InitializationError, OutOfMemoryError},
    initialization::{
      self,
      device::{Device, PhysicalDevice, Queues},
    },
    pipelines::{self, GraphicsPipeline},
    render_pass::{create_framebuffer, create_render_pass},
    render_targets::RenderTargets,
    SWAPCHAIN_IMAGE_USAGES,
  },
  utility::OnErr,
  INITIAL_WINDOW_HEIGHT, INITIAL_WINDOW_WIDTH, WINDOW_TITLE,
};

use super::{
  data::{compute::ComputeData, constant::ConstantData},
  descriptor_sets::DescriptorPool,
  initialization::Surface,
  pipelines::PipelineCreationError,
  render_object::RenderPosition,
  swapchain::{SwapchainCreationError, Swapchains},
  RenderInit, FRAMES_IN_FLIGHT,
};

#[derive(Debug, thiserror::Error)]
pub enum SwapchainRecreationError {
  #[error("Out of memory")]
  OutOfMemory(OutOfMemoryError),
  #[error("Failed to create a swapchain")]
  SwapchainError(SwapchainCreationError),
  #[error("Failed to create a pipeline")]
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
  pub device: Device,
  pub queues: Queues,

  pub window: Window,
  surface: Surface,

  pub swapchains: Swapchains,

  render_pass: vk::RenderPass,
  render_targets: RenderTargets,

  pipeline_cache: vk::PipelineCache,
  pipeline: GraphicsPipeline,
  pub command_pools: [GraphicsCommandBufferPool; FRAMES_IN_FLIGHT],

  constant_data: ConstantData,
  compute_data: ComputeData,
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

    let mut destructor: Destructor<13> = Destructor::new();

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
    )?;
    destructor.push(&swapchains);

    let render_pass =
      create_render_pass(&device).on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&render_pass);

    let render_targets = RenderTargets::new(&device, &physical_device, render_pass)
      .on_err(|_| unsafe { destructor.fire(&device) })?;
    log::debug!("Created render targets:\n{:#?}", render_targets);
    destructor.push(&render_targets);

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

    let constant_data = {
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

      let data = create_and_populate_constant_data(
        &device,
        &physical_device,
        &queues,
        &mut temp_transfer_pool,
        &mut temp_graphics_pool,
      )
      .on_err(|_| unsafe {
        temp_transfer_pool.destroy_self(&device);
        temp_graphics_pool.destroy_self(&device);
        destructor.fire(&device)
      })?;

      log::debug!("GPU data addresses: {:#?}", data);

      unsafe {
        temp_transfer_pool.destroy_self(&device);
        temp_graphics_pool.destroy_self(&device);
      }

      data
    };

    let compute_data = ComputeData::new(&device, &physical_device)?;

    let descriptor_pool = DescriptorPool::new(&device, constant_data.texture_view)
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
      constant_data,
      compute_data,
      render_pass,
      pipeline: graphics_pipeline,
      pipeline_cache,
      swapchains,
      descriptor_pool,
      render_targets,
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
      frame_i,
      &self.device,
      self.render_pass,
      &self.render_targets,
      self.swapchains.get_images()[image_i],
      self.swapchains.get_extent(),
      &self.pipeline,
      &self.descriptor_pool,
      &self.constant_data,
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

    // old swapchain becomes retired
    let changes = self.swapchains.recreate(
      &self.physical_device,
      &self.device,
      &self.surface,
      self.window.inner_size(),
      SWAPCHAIN_IMAGE_USAGES,
    )?;

    if !changes.format && !changes.extent {
      log::warn!("Recreating swapchain without any extent or format change");
    }

    Ok(())
  }

  // destroy old objects that resulted of a swapchain recreation
  // this should only be called when they stop being in use
  pub unsafe fn destroy_old(&mut self) {
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

      self.constant_data.destroy_self(&self.device);
      self.compute_data.destroy_self(&self.device);

      self.render_targets.destroy_self(&self.device);
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
