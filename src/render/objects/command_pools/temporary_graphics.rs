use std::ptr::{self};

use ash::vk;

use crate::render::objects::device::QueueFamilies;

pub struct TemporaryGraphicsCommandBufferPool {
  pool: vk::CommandPool,
  pub acquire_texture: vk::CommandBuffer,
}

impl TemporaryGraphicsCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Self {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.graphics.index);

    let buffers = super::allocate_primary_command_buffers(device, pool, 1);

    Self {
      pool,
      acquire_texture: buffers[0],
    }
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .expect("Failed to reset command pool");
  }

  pub unsafe fn record_acquire_texture(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    texture_image: vk::Image,
  ) {
    let cb = self.acquire_texture;

    let command_buffer_begin_info = vk::CommandBufferBeginInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
      p_next: ptr::null(),
      p_inheritance_info: ptr::null(),
      flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
    };
    device
      .begin_command_buffer(cb, &command_buffer_begin_info)
      .expect("Failed to start recording command buffer");

    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    let acquire_to_shader_read = vk::ImageMemoryBarrier {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags::NONE, // should be NONE for ownership acquire
      dst_access_mask: vk::AccessFlags::SHADER_READ,
      old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
      src_queue_family_index: queue_families.get_transfer_index(),
      dst_queue_family_index: queue_families.get_graphics_index(),
      image: texture_image,
      subresource_range,
    };
    device.cmd_pipeline_barrier(
      cb,
      vk::PipelineStageFlags::TRANSFER,
      vk::PipelineStageFlags::FRAGMENT_SHADER,
      vk::DependencyFlags::empty(),
      &[],
      &[],
      &[acquire_to_shader_read],
    );

    device
      .end_command_buffer(cb)
      .expect("Failed to finish recording command buffer")
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
