use ash::vk;
use std::ptr::{self, addr_of};

use crate::{
  command_pools::CommandPools,
  create_objs::{create_fence, create_semaphore},
  destroy,
  device::{create_logical_device, PhysicalDevice, Queues},
  device_destroyable::{DeviceManuallyDestroyed, ManuallyDestroyed},
  entry,
  errors::{InitializationError, OutOfMemoryError},
  gpu_data::GPUData,
  instance::create_instance,
  pipeline::GraphicsPipeline,
  pipeline_cache,
  render_pass::create_render_pass,
  utility::OnErr,
};

pub struct Renderer {
  _entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: crate::validation_layers::DebugUtils,
  physical_device: PhysicalDevice,
  device: ash::Device,
  queues: Queues,

  render_pass: vk::RenderPass,
  pipeline: GraphicsPipeline,
  command_pools: CommandPools,
  gpu_data: GPUData,
}

impl Renderer {
  #[cfg(feature = "vl")]
  pub fn initialize(
    image_width: u32,
    image_height: u32,
    buffer_size: u64,
  ) -> Result<Self, InitializationError> {
    let entry: ash::Entry = unsafe { entry::get_entry() };
    let (instance, debug_utils) = create_instance(&entry)?;

    let physical_device = match unsafe { PhysicalDevice::select(&instance) }
      .on_err(|_| unsafe { destroy!(&debug_utils, &instance) })?
    {
      Some(device) => device,
      None => {
        unsafe { destroy!(&debug_utils, &instance) };
        return Err(InitializationError::NoCompatibleDevices);
      }
    };

    let (device, queues) = create_logical_device(&instance, &physical_device)
      .on_err(|_| unsafe { destroy!(&debug_utils, &instance) })?;

    let render_pass = create_render_pass(&device)
      .on_err(|_| unsafe { destroy!(&device, &debug_utils, &instance) })?;

    log::info!("Creating pipeline cache");
    let (pipeline_cache, created_from_file) =
      pipeline_cache::create_pipeline_cache(&device, &physical_device).on_err(|_| unsafe {
        destroy!(&device => &render_pass, &device, &debug_utils, &instance)
      })?;
    if created_from_file {
      log::info!("Cache successfully created from an existing cache file");
    } else {
      log::info!("Cache initialized as empty");
    }

    log::debug!("Creating pipeline");
    let pipeline =
      GraphicsPipeline::create(&device, pipeline_cache, render_pass).on_err(|_| unsafe {
        destroy!(&device => &pipeline_cache, &render_pass, &device, &debug_utils, &instance)
      })?;

    // no more pipelines will be created, so might as well save and delete the cache
    log::info!("Saving pipeline cache");
    if let Err(err) = pipeline_cache::save_pipeline_cache(&device, &physical_device, pipeline_cache)
    {
      log::error!("Failed to save pipeline cache: {:?}", err);
    }
    unsafe {
      pipeline_cache.destroy_self(&device);
    }

    let command_pools = CommandPools::new(&device, &physical_device).on_err(|_| unsafe {
      destroy!(&device => &pipeline, &render_pass, &device, &debug_utils, &instance)
    })?;

    let gpu_data = GPUData::new(
      &device,
      &physical_device,
      render_pass,
      vk::Extent2D {
        width: image_width,
        height: image_height,
      },
      buffer_size,
    )
    .on_err(|_| unsafe { destroy!(&device => &command_pools, &pipeline, &render_pass, &device, &debug_utils, &instance) })?;

    Ok(Self {
      _entry: entry,
      instance,
      debug_utils,
      physical_device,
      device,
      queues,
      command_pools,
      gpu_data,
      render_pass,
      pipeline,
    })
  }

  #[cfg(not(feature = "vl"))]
  pub fn initialize(
    image_width: u32,
    image_height: u32,
    buffer_size: u64,
  ) -> Result<Self, InitializationError> {
    let entry: ash::Entry = unsafe { entry::get_entry() };
    let instance = create_instance(&entry)?;

    let physical_device = match unsafe { PhysicalDevice::select(&instance) }
      .on_err(|_| unsafe { destroy!(&instance) })?
    {
      Some(device) => device,
      None => {
        unsafe { destroy!(&instance) };
        return Err(InitializationError::NoCompatibleDevices);
      }
    };

    let (device, queues) = create_logical_device(&instance, &physical_device)
      .on_err(|_| unsafe { destroy!(&instance) })?;

    let command_pools = CommandPools::new(&device, &physical_device)
      .on_err(|_| unsafe { destroy!(&device, &instance) })?;

    let gpu_data = GPUData::new(
      &device,
      &physical_device,
      image_width,
      image_height,
      buffer_size,
    )
    .on_err(|_| unsafe { destroy!(&device => &command_pools, &device, &instance) })?;

    Ok(Self {
      _entry: entry,
      instance,
      physical_device,
      device,
      queues,
      command_pools,
      gpu_data,
    })
  }

  pub unsafe fn record_work(&mut self) -> Result<(), OutOfMemoryError> {
    self.command_pools.graphics_pool.reset(&self.device)?;
    self.command_pools.graphics_pool.record_triangle(&self.device, 
      &self.physical_device.queue_families,
      self.render_pass,
      &self.pipeline,
      &self.gpu_data.triangle_image,
      &self.gpu_data.triangle_model
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
