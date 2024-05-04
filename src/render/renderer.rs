use ash::vk;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
  marker::PhantomData,
  mem::MaybeUninit,
  ptr::{self, addr_of},
};
use winit::{
  dpi::PhysicalSize,
  event_loop::EventLoopWindowTarget,
  window::{Window, WindowBuilder},
};

use crate::{
  destroy,
  render::{
    command_pools::CommandPools,
    create_objs::{create_fence, create_semaphore},
    device_destroyable::{DeviceManuallyDestroyed, ManuallyDestroyed},
    errors::{InitializationError, OutOfMemoryError},
    gpu_data::GPUData,
    initialization::{
      self,
      device::{create_logical_device, PhysicalDevice, Queues},
    },
    pipelines::{self, GraphicsPipeline},
    render_pass::create_render_pass,
  },
  utility::OnErr,
  INITIAL_WINDOW_HEIGHT, INITIAL_WINDOW_WIDTH, WINDOW_TITLE,
};

use super::{
  descriptor_sets::DescriptorPool, initialization::Surface, swapchain::Swapchains, RenderInit,
  FRAMES_IN_FLIGHT,
};

pub struct Renderer {
  _entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: initialization::DebugUtils,
  physical_device: PhysicalDevice,
  device: ash::Device,
  queues: Queues,

  _window: Window,
  surface: Surface,

  pub swapchains: Swapchains,
  render_pass: vk::RenderPass,
  framebuffers: [vk::Framebuffer; FRAMES_IN_FLIGHT],

  descriptor_pool: DescriptorPool,
  pipeline: GraphicsPipeline,
  command_pools: CommandPools,

  gpu_data: GPUData,
}

const DESTROYABLE_COUNT: usize = 10;
struct Destructor {
  objs: [*const dyn DeviceManuallyDestroyed; DESTROYABLE_COUNT],
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
    self.i += 1;
    self.objs[self.i] = ptr;
  }

  pub unsafe fn fire(self, device: &ash::Device) {
    for i in (0..self.len).rev() {
      self.objs[i].as_ref().unwrap().destroy_self(device);
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
    destructor.push(instance);
    #[cfg(feature = "vl")]
    destructor.push(debug_utils);

    let surface = Surface::new(
      &entry,
      &instance,
      target.display_handle(),
      window.window_handle(),
    )
    .on_err(|_| destroy_instance())?;
    destructor.push(surface);

    let physical_device =
      match unsafe { PhysicalDevice::select(&instance) }.on_err(|_| destroy_instance())? {
        Some(device) => device,
        None => {
          destroy_instance();
          return Err(InitializationError::NoCompatibleDevices);
        }
      };

    let (device, queues) =
      create_logical_device(&instance, &physical_device).on_err(|_| destroy_instance())?;
    destructor.push(device);

    let swapchains = Swapchains::new(
      &instance,
      &physical_device,
      &device,
      &surface,
      window.inner_size(),
    )?;
    destructor.push(swapchains);

    let render_pass = create_render_pass(&device, swapchains.get_format())
      .on_err(|_| unsafe { destructor.fire(&device) })?;

    log::info!("Creating pipeline cache");
    let (pipeline_cache, created_from_file) =
      pipelines::create_pipeline_cache(&device, &physical_device).on_err(|_| unsafe {
        destroy!(&device => &render_pass, &device);
        destroy_instance();
      })?;
    if created_from_file {
      log::info!("Cache successfully created from an existing cache file");
    } else {
      log::info!("Cache initialized as empty");
    }

    log::debug!("Creating pipeline");
    let pipeline = GraphicsPipeline::create(&device, pipeline_cache, render_pass)
      .on_err(|_| unsafe { destructor.fire(&device) })?;

    let mut command_pools = CommandPools::new(&device, &physical_device)
      .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(command_pools);

    let mut gpu_data = GPUData::new(&device, &physical_device, render_pass).on_err(|_| unsafe {
      destroy!(&device => &command_pools, &pipeline, &render_pass, &device);
      destroy_instance();
    })?;

    gpu_data.initialize_memory(
      &device,
      &physical_device,
      &queues,
      &mut command_pools.transfer_pool,
    )?;

    Ok(Self {
      _window: window,
      surface,
      _entry: entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
      physical_device,
      device,
      queues,
      command_pools,
      gpu_data,
      render_pass,
      pipeline,
      swapchains,
      descriptor_pool,
      framebuffers,
    })
  }

  pub unsafe fn record_work(&mut self) -> Result<(), OutOfMemoryError> {
    self.command_pools.graphics_pool.reset(&self.device)?;
    self.command_pools.graphics_pool.record_triangle(
      &self.device,
      &self.physical_device.queue_families,
      self.render_pass,
      &self.pipeline,
      &self.gpu_data.triangle_image,
      &self.gpu_data.triangle_model,
    )?;

    self.command_pools.transfer_pool.reset(&self.device)?;
    self.command_pools.transfer_pool.record_copy_img_to_buffer(
      &self.device,
      &self.physical_device.queue_families,
      self.gpu_data.triangle_image.image,
      self.gpu_data.final_buffer.buffer,
    )?;

    Ok(())
  }

  // can return vk::Result::ERROR_DEVICE_LOST
  pub fn submit_and_wait(&self) -> Result<(), vk::Result> {
    let image_clear_finished = create_semaphore(&self.device)?;
    let all_done = create_fence(&self.device)
      .on_err(|_| unsafe { destroy!(&self.device => &image_clear_finished) })?;

    let clear_image_submit = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 0,
      p_wait_semaphores: ptr::null(),
      p_wait_dst_stage_mask: ptr::null(),
      command_buffer_count: 1,
      p_command_buffers: addr_of!(self.command_pools.graphics_pool.triangle),
      signal_semaphore_count: 1,
      p_signal_semaphores: addr_of!(image_clear_finished),
      _marker: PhantomData,
    };
    let wait_for = vk::PipelineStageFlags::TRANSFER;
    let transfer_image_submit = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 1,
      p_wait_semaphores: addr_of!(image_clear_finished),
      p_wait_dst_stage_mask: addr_of!(wait_for),
      command_buffer_count: 1,
      p_command_buffers: addr_of!(self.command_pools.transfer_pool.copy_image_to_buffer),
      signal_semaphore_count: 0,
      p_signal_semaphores: ptr::null(),
      _marker: PhantomData,
    };

    let destroy_objs = || unsafe { destroy!(&self.device => &image_clear_finished, &all_done) };

    unsafe {
      self
        .device
        .queue_submit(
          self.queues.graphics,
          &[clear_image_submit],
          vk::Fence::null(),
        )
        .on_err(|_| destroy_objs())?;
      self
        .device
        .queue_submit(self.queues.transfer, &[transfer_image_submit], all_done)
        .on_err(|_| destroy_objs())?;

      self
        .device
        .wait_for_fences(&[all_done], true, u64::MAX)
        .on_err(|_| destroy_objs())?;
    }

    destroy_objs();

    Ok(())
  }

  pub unsafe fn get_resulting_data<F: FnOnce(&[u8])>(&self, f: F) -> Result<(), vk::Result> {
    self.gpu_data.get_buffer_data(&self.device, f)
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

      destroy!(&self.device => &self.command_pools, &self.gpu_data, &self.pipeline, &self.render_pass);

      ManuallyDestroyed::destroy_self(&self.device);

      #[cfg(feature = "vl")]
      {
        ManuallyDestroyed::destroy_self(&self.debug_utils);
      }
      ManuallyDestroyed::destroy_self(&self.instance);
    }
  }
}
