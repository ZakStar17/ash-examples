use std::ptr;

use ash::vk;

use crate::render::objects::device::QueueFamilies;

pub struct TransferCommandBufferPool {
  pool: vk::CommandPool,
  pub copy_buffers: vk::CommandBuffer,
  pub load_texture: vk::CommandBuffer,
}

impl TransferCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Self {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_transfer_index());

    let buffers = super::allocate_primary_command_buffers(device, pool, 2);

    Self {
      pool,
      copy_buffers: buffers[0],
      load_texture: buffers[1],
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

  pub unsafe fn record_load_texture(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    staging_buffer: vk::Buffer,
    texture_image: vk::Image,
    texture_width: u32,
    texture_height: u32,
  ) {
    let cb = self.load_texture;

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

    let transfer_dst_layout = vk::ImageMemoryBarrier {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags::NONE,
      dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
      old_layout: vk::ImageLayout::UNDEFINED,
      new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      image: texture_image,
      subresource_range,
    };
    device.cmd_pipeline_barrier(
      cb,
      vk::PipelineStageFlags::TRANSFER, // can be NONE
      vk::PipelineStageFlags::TRANSFER,
      vk::DependencyFlags::empty(),
      &[],
      &[],
      &[transfer_dst_layout],
    );

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

    let release_to_shader_read = vk::ImageMemoryBarrier {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
      dst_access_mask: vk::AccessFlags::NONE, // should be NONE for ownership release
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
      vk::PipelineStageFlags::TRANSFER,
      vk::DependencyFlags::empty(),
      &[],
      &[],
      &[release_to_shader_read],
    );

    device
      .end_command_buffer(cb)
      .expect("Failed to finish recording command buffer")
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
