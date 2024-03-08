use std::ptr;

use ash::vk;

mod compute;
mod transfer;

pub use compute::ComputeCommandBufferPool;
pub use transfer::TransferCommandBufferPool;

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
