use ash::vk;
use std::{
  ops::BitOr,
  ptr::{self, addr_of},
};

use crate::{
  allocator::allocate_and_bind_memory,
  command_pools::{ComputeCommandBufferPool, TransferCommandBufferPool},
  device::{create_logical_device, PhysicalDevice, Queues},
  entry,
  errors::{AllocationError, InitializationError, OutOfMemoryError},
  instance::create_instance,
  utility::OnErr,
  IMAGE_FORMAT,
};

fn create_semaphore(device: &ash::Device) -> Result<vk::Semaphore, OutOfMemoryError> {
  let create_info = vk::SemaphoreCreateInfo {
    s_type: vk::StructureType::SEMAPHORE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::SemaphoreCreateFlags::empty(),
  };
  unsafe { device.create_semaphore(&create_info, None) }.map_err(|err| err.into())
}

fn create_fence(device: &ash::Device) -> Result<vk::Fence, OutOfMemoryError> {
  let create_info = vk::FenceCreateInfo {
    s_type: vk::StructureType::FENCE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::FenceCreateFlags::empty(),
  };
  unsafe { device.create_fence(&create_info, None) }.map_err(|err| err.into())
}

fn create_buffer(
  device: &ash::Device,
  size: u64,
  usage: vk::BufferUsageFlags,
) -> Result<vk::Buffer, OutOfMemoryError> {
  let create_info = vk::BufferCreateInfo {
    s_type: vk::StructureType::BUFFER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::BufferCreateFlags::empty(),
    size,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(),
  };
  unsafe { device.create_buffer(&create_info, None) }.map_err(|err| err.into())
}

pub fn create_image(
  device: &ash::Device,
  width: u32,
  height: u32,
  usage: vk::ImageUsageFlags,
) -> Result<vk::Image, OutOfMemoryError> {
  // 1 color layer 2d image
  let create_info = vk::ImageCreateInfo {
    s_type: vk::StructureType::IMAGE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::ImageCreateFlags::empty(),
    image_type: vk::ImageType::TYPE_2D,
    format: IMAGE_FORMAT,
    extent: vk::Extent3D {
      width,
      height,
      depth: 1,
    },
    mip_levels: 1,
    array_layers: 1,
    samples: vk::SampleCountFlags::TYPE_1,
    tiling: vk::ImageTiling::OPTIMAL,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(), // ignored if sharing mode is exclusive
    initial_layout: vk::ImageLayout::UNDEFINED,
  };

  unsafe { device.create_image(&create_info, None) }.map_err(|err| err.into())
}

pub struct Renderer {
  _entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: crate::validation_layers::DebugUtils,
  physical_device: PhysicalDevice,
  device: ash::Device,
  queues: Queues,
  command_pools: CommandPools,
  gpu_data: GPUData,
}

struct CommandPools {
  compute_pool: ComputeCommandBufferPool,
  transfer_pool: TransferCommandBufferPool,
}

struct GPUData {
  local_image: vk::Image,
  local_image_memory: vk::DeviceMemory,
  host_buffer: vk::Buffer,
  host_buffer_size: u64,
  host_buffer_memory: vk::DeviceMemory,
}

impl Renderer {
  pub fn initialize(
    image_width: u32,
    image_height: u32,
    buffer_size: u64,
  ) -> Result<Self, InitializationError> {
    let entry: ash::Entry = unsafe { entry::get_entry() };

    #[cfg(feature = "vl")]
    let (instance, mut debug_utils) = create_instance(&entry)?;
    #[cfg(not(feature = "vl"))]
    let instance = create_instance(&entry)?;

    let mut destroy_instance = || unsafe {
      #[cfg(feature = "vl")]
      debug_utils.destroy_self();
      instance.destroy_instance(None);
    };

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

    let mut command_pools = CommandPools::new(&device, &physical_device).on_err(|_| unsafe {
      device.destroy_device(None);
      destroy_instance();
    })?;

    let gpu_data = GPUData::new(
      &device,
      &physical_device,
      image_width,
      image_height,
      buffer_size,
    )
    .on_err(|_| unsafe {
      command_pools.destroy_self(&device);
      device.destroy_device(None);
      destroy_instance();
    })?;

    Ok(Self {
      _entry: entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
      physical_device,
      device,
      queues,
      command_pools,
      gpu_data,
    })
  }

  pub unsafe fn record_work(&mut self) -> Result<(), OutOfMemoryError> {
    self.command_pools.compute_pool.reset(&self.device)?;
    self.command_pools.compute_pool.record_clear_img(
      &self.device,
      &self.physical_device.queue_families,
      self.gpu_data.local_image,
    )?;

    self.command_pools.transfer_pool.reset(&self.device)?;
    self.command_pools.transfer_pool.record_copy_img_to_buffer(
      &self.device,
      &self.physical_device.queue_families,
      self.gpu_data.local_image,
      self.gpu_data.host_buffer,
    )?;

    Ok(())
  }

  // can return vk::Result::ERROR_DEVICE_LOST
  pub fn submit_and_wait(&self) -> Result<(), vk::Result> {
    let image_clear_finished = create_semaphore(&self.device)?;
    let clear_image_submit = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 0,
      p_wait_semaphores: ptr::null(),
      p_wait_dst_stage_mask: ptr::null(),
      command_buffer_count: 1,
      p_command_buffers: addr_of!(self.command_pools.compute_pool.clear_img),
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
      p_command_buffers: addr_of!(self.command_pools.transfer_pool.copy_to_host),
      signal_semaphore_count: 0,
      p_signal_semaphores: ptr::null(),
    };

    let finished = create_fence(&self.device)
      .on_err(|_| unsafe { self.device.destroy_semaphore(image_clear_finished, None) })?;

    let destroy_objs = || unsafe {
      self.device.destroy_fence(finished, None);
      self.device.destroy_semaphore(image_clear_finished, None);
    };

    unsafe {
      self
        .device
        .queue_submit(
          self.queues.compute,
          &[clear_image_submit],
          vk::Fence::null(),
        )
        .on_err(|_| destroy_objs())?;
      self
        .device
        .queue_submit(self.queues.transfer, &[transfer_image_submit], finished)
        .on_err(|_| destroy_objs())?;

      self
        .device
        .wait_for_fences(&[finished], true, u64::MAX)
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

      self.command_pools.destroy_self(&self.device);
      self.gpu_data.destroy_self(&self.device);

      self.device.destroy_device(None);

      #[cfg(feature = "vl")]
      {
        self.debug_utils.destroy_self();
      }
      self.instance.destroy_instance(None);
    }
  }
}

impl CommandPools {
  pub fn new(device: &ash::Device, physical_device: &PhysicalDevice) -> Result<Self, vk::Result> {
    let mut compute_pool =
      ComputeCommandBufferPool::create(&device, &physical_device.queue_families)?;
    let transfer_pool =
      match TransferCommandBufferPool::create(&device, &physical_device.queue_families) {
        Ok(pool) => pool,
        Err(err) => {
          unsafe {
            compute_pool.destroy_self(device);
          }
          return Err(err);
        }
      };
    Ok(Self {
      compute_pool,
      transfer_pool,
    })
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    self.compute_pool.destroy_self(device);
    self.transfer_pool.destroy_self(device);
  }
}

impl GPUData {
  pub fn new(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    image_width: u32,
    image_height: u32,
    buffer_size: u64,
  ) -> Result<Self, AllocationError> {
    // GPU image with DEVICE_LOCAL flags
    let local_image = create_image(
      &device,
      image_width,
      image_height,
      vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::TRANSFER_DST),
    )?;
    log::debug!("Allocating memory for local image");
    let local_image_memory = match allocate_and_bind_memory(
      &device,
      &physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &[],
      &[],
      &[local_image],
      &[unsafe { device.get_image_memory_requirements(local_image) }],
    )
    .or_else(|err| {
      log::warn!(
        "Failed to allocate optimal memory for local image:\n{:?}",
        err
      );
      allocate_and_bind_memory(
        &device,
        &physical_device,
        vk::MemoryPropertyFlags::empty(),
        &[],
        &[],
        &[local_image],
        &[unsafe { device.get_image_memory_requirements(local_image) }],
      )
    }) {
      Ok(alloc) => alloc.memory,
      Err(err) => {
        unsafe {
          device.destroy_image(local_image, None);
        }
        return Err(err);
      }
    };

    let host_buffer = match create_buffer(&device, buffer_size, vk::BufferUsageFlags::TRANSFER_DST)
    {
      Ok(buffer) => buffer,
      Err(err) => {
        unsafe {
          device.free_memory(local_image_memory, None);
          device.destroy_image(local_image, None);
        }
        return Err(err.into());
      }
    };
    log::debug!("Allocating memory for host buffer");
    let host_buffer_memory = match allocate_and_bind_memory(
      &device,
      &physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_CACHED),
      &[host_buffer],
      &[unsafe { device.get_buffer_memory_requirements(host_buffer) }],
      &[],
      &[],
    )
    .or_else(|err| {
      log::warn!(
        "Failed to allocate optimal memory for host buffer:\n{:?}",
        err
      );
      allocate_and_bind_memory(
        &device,
        &physical_device,
        vk::MemoryPropertyFlags::HOST_VISIBLE,
        &[host_buffer],
        &[unsafe { device.get_buffer_memory_requirements(host_buffer) }],
        &[],
        &[],
      )
    }) {
      Ok(alloc) => alloc.memory,
      Err(err) => {
        unsafe {
          device.free_memory(local_image_memory, None);
          device.destroy_image(local_image, None);
          device.destroy_buffer(host_buffer, None);
        }
        return Err(err);
      }
    };

    Ok(Self {
      local_image,
      local_image_memory,
      host_buffer,
      host_buffer_size: buffer_size,
      host_buffer_memory,
    })
  }

  // map can fail with vk::Result::ERROR_MEMORY_MAP_FAILED
  // in most cases it may be possible to try mapping again a smaller range
  pub unsafe fn get_buffer_data<F: FnOnce(&[u8])>(
    &self,
    device: &ash::Device,
    f: F,
  ) -> Result<(), vk::Result> {
    let ptr = device.map_memory(
      self.host_buffer_memory,
      0,
      // if size is not vk::WHOLE_SIZE, mapping should follow alignments
      vk::WHOLE_SIZE,
      vk::MemoryMapFlags::empty(),
    )? as *const u8;
    let data = std::slice::from_raw_parts(ptr, self.host_buffer_size as usize);

    f(data);

    unsafe {
      device.unmap_memory(self.host_buffer_memory);
    }

    Ok(())
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_image(self.local_image, None);
    device.free_memory(self.local_image_memory, None);
    device.destroy_buffer(self.host_buffer, None);
    device.free_memory(self.host_buffer_memory, None);
  }
}
