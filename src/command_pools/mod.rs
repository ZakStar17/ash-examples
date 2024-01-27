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
) -> vk::CommandPool {
  let command_pool_create_info = vk::CommandPoolCreateInfo {
    s_type: vk::StructureType::COMMAND_POOL_CREATE_INFO,
    p_next: ptr::null(),
    flags,
    queue_family_index,
  };

  log::debug!("Creating command pool");
  unsafe {
    device
      .create_command_pool(&command_pool_create_info, None)
      .expect("Failed to create Command Pool!")
  }
}

fn allocate_primary_command_buffers(
  device: &ash::Device,
  command_pool: vk::CommandPool,
  command_buffer_count: u32,
) -> Vec<vk::CommandBuffer> {
  let allocate_info = vk::CommandBufferAllocateInfo {
    s_type: vk::StructureType::COMMAND_BUFFER_ALLOCATE_INFO,
    p_next: ptr::null(),
    command_buffer_count,
    command_pool,
    level: vk::CommandBufferLevel::PRIMARY,
  };

  log::debug!("Allocating command buffers");
  unsafe {
    device
      .allocate_command_buffers(&allocate_info)
      .expect("Failed to allocate command buffers")
  }
}
