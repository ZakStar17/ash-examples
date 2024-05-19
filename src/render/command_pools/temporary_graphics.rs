use std::{marker::PhantomData, ptr};

use ash::vk;

use crate::render::{
  device_destroyable::DeviceManuallyDestroyed, errors::OutOfMemoryError,
  initialization::device::QueueFamilies,
};

use super::dependency_info;

pub struct TemporaryGraphicsCommandPool {
  pool: vk::CommandPool,
  pub acquire_texture: vk::CommandBuffer,
}

impl TemporaryGraphicsCommandPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Result<Self, vk::Result> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.graphics.index)?;

    let buffers = super::allocate_primary_command_buffers(device, pool, 1)?;

    Ok(Self {
      pool,
      acquire_texture: buffers[0],
    })
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) -> Result<(), OutOfMemoryError> {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .map_err(|err| err.into())
  }

  pub unsafe fn record_acquire_texture(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    texture_image: vk::Image,
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.acquire_texture;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    let acquire_to_shader_read = vk::ImageMemoryBarrier2 {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
      p_next: ptr::null(),
      src_stage_mask: vk::PipelineStageFlags2::TRANSFER,
      dst_stage_mask: vk::PipelineStageFlags2::FRAGMENT_SHADER,
      src_access_mask: vk::AccessFlags2::NONE, // NONE for ownership acquire
      dst_access_mask: vk::AccessFlags2::SHADER_SAMPLED_READ,
      old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
      src_queue_family_index: queue_families.get_transfer_index(),
      dst_queue_family_index: queue_families.get_graphics_index(),
      image: texture_image,
      subresource_range,
      _marker: PhantomData,
    };
    device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[acquire_to_shader_read]));

    device.end_command_buffer(cb)?;
    Ok(())
  }
}

impl DeviceManuallyDestroyed for TemporaryGraphicsCommandPool {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
