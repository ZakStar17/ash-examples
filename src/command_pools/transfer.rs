use std::ptr;

use ash::vk;

use crate::{
  device::QueueFamilies, device_destroyable::DeviceManuallyDestroyed, errors::OutOfMemoryError,
  IMAGE_HEIGHT, IMAGE_WIDTH,
};

use super::dependency_info;

pub struct TransferCommandBufferPool {
  pool: vk::CommandPool,
  // separated for simplicity
  pub copy_image_to_buffer: vk::CommandBuffer,
  pub copy_buffers_to_buffers: vk::CommandBuffer,
}

impl TransferCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Result<Self, vk::Result> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_transfer_index())?;

    let command_buffers = super::allocate_primary_command_buffers(device, pool, 2)?;
    let copy_image_to_buffer = command_buffers[0];
    let copy_buffers_to_buffers = command_buffers[1];

    Ok(Self {
      pool,
      copy_image_to_buffer,
      copy_buffers_to_buffers,
    })
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) -> Result<(), vk::Result> {
    device.reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
  }

  pub unsafe fn record_copy_img_to_buffer(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    src_image: vk::Image,
    dst_buffer: vk::Buffer,
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.copy_image_to_buffer;
    let begin_info = vk::CommandBufferBeginInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
      p_next: ptr::null(),
      flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
      p_inheritance_info: ptr::null(),
    };
    device.begin_command_buffer(cb, &begin_info)?;

    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    if queue_families.get_graphics_index() != queue_families.get_transfer_index() {
      // matches to release found in compute
      let src_acquire = vk::ImageMemoryBarrier2 {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::NONE, // NONE for ownership acquire,
        dst_access_mask: vk::AccessFlags2::TRANSFER_READ,
        src_stage_mask: vk::PipelineStageFlags2::TRANSFER, // from semaphore
        dst_stage_mask: vk::PipelineStageFlags2::COPY,
        old_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        src_queue_family_index: queue_families.get_graphics_index(),
        dst_queue_family_index: queue_families.get_transfer_index(),
        image: src_image,
        subresource_range,
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[src_acquire]));
    }

    // 1 color layer
    let subresource_layers = vk::ImageSubresourceLayers {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      mip_level: 0,
      base_array_layer: 0,
      layer_count: 1,
    };
    // full image
    let copy_region = vk::BufferImageCopy {
      image_subresource: subresource_layers,
      image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
      image_extent: vk::Extent3D {
        width: IMAGE_WIDTH,
        height: IMAGE_HEIGHT,
        depth: 1,
      },
      buffer_offset: 0,
      buffer_image_height: 0, // densely packed
      buffer_row_length: 0,
    };
    device.cmd_copy_image_to_buffer(
      cb,
      src_image,
      vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
      dst_buffer,
      &[copy_region],
    );

    // flush memory to host
    let flush_host = vk::BufferMemoryBarrier2 {
      s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
      dst_access_mask: vk::AccessFlags2::HOST_READ,
      src_stage_mask: vk::PipelineStageFlags2::COPY,
      dst_stage_mask: vk::PipelineStageFlags2::HOST,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      buffer: dst_buffer,
      offset: 0,
      size: vk::WHOLE_SIZE,
    };
    device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[flush_host], &[]));

    device.end_command_buffer(cb)?;

    Ok(())
  }

  pub unsafe fn record_copy_buffers_to_buffers(
    &mut self,
    device: &ash::Device,
    copy_infos: &[vk::CopyBufferInfo2],
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.copy_buffers_to_buffers;

    let command_buffer_begin_info = vk::CommandBufferBeginInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
      p_next: ptr::null(),
      p_inheritance_info: ptr::null(),
      flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
    };
    device.begin_command_buffer(cb, &command_buffer_begin_info)?;

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
