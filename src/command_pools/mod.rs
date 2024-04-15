use std::ptr;

use ash::vk;

mod graphics;
mod transfer;

pub use graphics::GraphicsCommandBufferPool;
pub use transfer::TransferCommandBufferPool;

use crate::{device::PhysicalDevice, device_destroyable::DeviceManuallyDestroyed};

pub fn create_command_pool(
  device: &ash::Device,
  flags: vk::CommandPoolCreateFlags,
  queue_family_index: u32,
) -> Result<vk::CommandPool, vk::Result> {
  let command_pool_create_info = vk::CommandPoolCreateInfo {
    s_type: vk::StructureType::COMMAND_POOL_CREATE_INFO,
    p_next: ptr::null(),
    flags,
    queue_family_index,
  };
  log::debug!("Creating command pool");
  unsafe { device.create_command_pool(&command_pool_create_info, None) }
}

fn allocate_primary_command_buffers(
  device: &ash::Device,
  command_pool: vk::CommandPool,
  command_buffer_count: u32,
) -> Result<Vec<vk::CommandBuffer>, vk::Result> {
  let allocate_info = vk::CommandBufferAllocateInfo {
    s_type: vk::StructureType::COMMAND_BUFFER_ALLOCATE_INFO,
    p_next: ptr::null(),
    command_buffer_count,
    command_pool,
    level: vk::CommandBufferLevel::PRIMARY,
  };

  log::debug!("Allocating command buffers");
  unsafe { device.allocate_command_buffers(&allocate_info) }
}

fn dependency_info(
  memory: &[vk::MemoryBarrier2],
  buffer: &[vk::BufferMemoryBarrier2],
  image: &[vk::ImageMemoryBarrier2],
) -> vk::DependencyInfo {
  vk::DependencyInfo {
    s_type: vk::StructureType::DEPENDENCY_INFO,
    p_next: ptr::null(),
    dependency_flags: vk::DependencyFlags::empty(),
    memory_barrier_count: memory.len() as u32,
    p_memory_barriers: memory.as_ptr(),
    buffer_memory_barrier_count: buffer.len() as u32,
    p_buffer_memory_barriers: buffer.as_ptr(),
    image_memory_barrier_count: image.len() as u32,
    p_image_memory_barriers: image.as_ptr(),
  }
}

pub struct CommandPools {
  pub graphics_pool: GraphicsCommandBufferPool,
  pub transfer_pool: TransferCommandBufferPool,
}

impl CommandPools {
  pub fn new(device: &ash::Device, physical_device: &PhysicalDevice) -> Result<Self, vk::Result> {
    let graphics_pool =
      GraphicsCommandBufferPool::create(&device, &physical_device.queue_families)?;
    let transfer_pool =
      match TransferCommandBufferPool::create(&device, &physical_device.queue_families) {
        Ok(pool) => pool,
        Err(err) => {
          unsafe {
            graphics_pool.destroy_self(device);
          }
          return Err(err);
        }
      };
    Ok(Self {
      graphics_pool,
      transfer_pool,
    })
  }
}

impl DeviceManuallyDestroyed for CommandPools {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.graphics_pool.destroy_self(device);
    self.transfer_pool.destroy_self(device);
  }
}
