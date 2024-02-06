use std::ptr;

use ash::vk;

use crate::render::objects::device::QueueFamilies;

pub struct TransferCommandBufferPool {
  pool: vk::CommandPool,
  pub copy_buffers: vk::CommandBuffer,
}

impl TransferCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Self {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_transfer_index());

    let buffers = super::allocate_primary_command_buffers(device, pool, 1);

    Self {
      pool,
      copy_buffers: buffers[0],
    }
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .expect("Failed to reset command pool");
  }

  pub unsafe fn record_copy_buffers(
    &mut self,
    device: &ash::Device,
    copy_infos: &[vk::CopyBufferInfo2],
  ) {
    let cb = self.copy_buffers;

    let command_buffer_begin_info = vk::CommandBufferBeginInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
      p_next: ptr::null(),
      p_inheritance_info: ptr::null(),
      flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
    };
    device
      .begin_command_buffer(cb, &command_buffer_begin_info)
      .expect("Failed to start recording command buffer");

    for copy_info in copy_infos {
      device.cmd_copy_buffer2(cb, copy_info);
    }

    device
      .end_command_buffer(cb)
      .expect("Failed to finish recording command buffer")
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
