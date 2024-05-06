use std::{marker::PhantomData, ptr};

use ash::vk;

use crate::render::{
  device_destroyable::DeviceManuallyDestroyed, errors::OutOfMemoryError,
  initialization::device::QueueFamilies,
};

use super::dependency_info;

pub struct TransferCommandBufferPool {
  pool: vk::CommandPool,
  // separated for simplicity
  pub load_texture: vk::CommandBuffer,
  pub copy_buffers_to_buffers: vk::CommandBuffer,
}

impl TransferCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Result<Self, vk::Result> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_transfer_index())?;

    let command_buffers = super::allocate_primary_command_buffers(device, pool, 2)?;
    let load_texture = command_buffers[0];
    let copy_buffers_to_buffers = command_buffers[1];

    Ok(Self {
      pool,
      load_texture,
      copy_buffers_to_buffers,
    })
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) -> Result<(), vk::Result> {
    device.reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
  }

  // copy texture buffer data to an image
  // pairs with TemporaryGraphicsCommandPool::record_acquire_texture()
  pub unsafe fn record_load_texture(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    staging_buffer: vk::Buffer,
    texture_image: vk::Image,
    texture_width: u32,
    texture_height: u32,
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.load_texture;
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

    // change image layout to TRANSFER_DST before copy operation
    let transfer_dst_layout = vk::ImageMemoryBarrier2 {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_stage_mask: vk::PipelineStageFlags2::NONE,
      dst_stage_mask: vk::PipelineStageFlags2::COPY,
      src_access_mask: vk::AccessFlags2::NONE,
      dst_access_mask: vk::AccessFlags2::NONE,
      old_layout: vk::ImageLayout::UNDEFINED,
      new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      image: texture_image,
      subresource_range,
      _marker: PhantomData,
    };
    device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[transfer_dst_layout]));

    let copy_region = vk::BufferImageCopy {
      buffer_offset: 0,
      buffer_row_length: 0,
      buffer_image_height: 0,
      image_subresource: vk::ImageSubresourceLayers {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        mip_level: 0,
        base_array_layer: 0,
        layer_count: 1,
      },
      image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
      image_extent: vk::Extent3D {
        width: texture_width,
        height: texture_height,
        depth: 1,
      },
    };
    device.cmd_copy_buffer_to_image(
      cb,
      staging_buffer,
      texture_image,
      vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      &[copy_region],
    );

    let mut shader_read_layout = vk::ImageMemoryBarrier2 {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_stage_mask: vk::PipelineStageFlags2::COPY,
      dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
      src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
      dst_access_mask: vk::AccessFlags2::SHADER_SAMPLED_READ,
      old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      image: texture_image,
      subresource_range,
      _marker: PhantomData,
    };
    if queue_families.get_graphics_index() != queue_families.get_transfer_index() {
      // release if queues are different
      shader_read_layout.dst_access_mask = vk::AccessFlags2::NONE;
      shader_read_layout.src_queue_family_index = queue_families.get_transfer_index();
      shader_read_layout.dst_queue_family_index = queue_families.get_graphics_index();
    }
    device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[shader_read_layout]));

    device.end_command_buffer(cb)?;
    Ok(())
  }

  pub unsafe fn record_copy_buffers_to_buffers(
    &mut self,
    device: &ash::Device,
    copy_infos: &[vk::CopyBufferInfo2],
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.copy_buffers_to_buffers;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    for copy_info in copy_infos {
      device.cmd_copy_buffer2(cb, copy_info);
    }

    device.end_command_buffer(cb).map_err(|err| err.into())
  }
}

impl DeviceManuallyDestroyed for TransferCommandBufferPool {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
