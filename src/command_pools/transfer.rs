use std::ptr;

use ash::vk;

use crate::{device::QueueFamilies, IMAGE_HEIGHT, IMAGE_WIDTH};

pub struct TransferCommandBufferPool {
  pool: vk::CommandPool,
  pub copy_to_host: vk::CommandBuffer,
  pub copy_buffers: vk::CommandBuffer,
}

impl TransferCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Self {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_transfer_index());

    let buffers = super::allocate_primary_command_buffers(device, pool, 2);

    Self {
      pool,
      copy_to_host: buffers[0],
      copy_buffers: buffers[1],
    }
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .expect("Failed to reset command pool");
  }

  pub unsafe fn record_copy_img_to_host(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    src_image: vk::Image,
    dst_image: vk::Image,
  ) {
    let begin_info = vk::CommandBufferBeginInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
      p_next: ptr::null(),
      flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
      p_inheritance_info: ptr::null(),
    };
    device
      .begin_command_buffer(self.copy_to_host, &begin_info)
      .expect("Failed to begin recording command buffer");

    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    let src_acquire = vk::ImageMemoryBarrier {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags::NONE, // should be NONE for ownership acquire
      dst_access_mask: vk::AccessFlags::TRANSFER_READ,
      old_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
      new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
      src_queue_family_index: queue_families.get_graphics_index(),
      dst_queue_family_index: queue_families.get_transfer_index(),
      image: src_image,
      subresource_range,
    };
    // change destination image layout and access flags to transfer write
    let dst_transfer_dst_layout = vk::ImageMemoryBarrier {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags::empty(),
      dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
      old_layout: vk::ImageLayout::UNDEFINED,
      new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      image: dst_image,
      subresource_range,
    };
    device.cmd_pipeline_barrier(
      self.copy_to_host,
      vk::PipelineStageFlags::TRANSFER,
      vk::PipelineStageFlags::TRANSFER,
      vk::DependencyFlags::empty(),
      &[],
      &[],
      &[src_acquire, dst_transfer_dst_layout],
    );

    // 1 color layer
    let subresource_layers = vk::ImageSubresourceLayers {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      mip_level: 0,
      base_array_layer: 0,
      layer_count: 1,
    };
    // full image
    let copy_region = vk::ImageCopy {
      src_subresource: subresource_layers,
      src_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
      dst_subresource: subresource_layers,
      dst_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
      extent: vk::Extent3D {
        width: IMAGE_WIDTH,
        height: IMAGE_HEIGHT,
        depth: 1,
      },
    };
    device.cmd_copy_image(
      self.copy_to_host,
      src_image,
      vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
      dst_image,
      vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      &[copy_region],
    );

    // change destination image access flags to host read
    let make_dst_host_accessible = vk::ImageMemoryBarrier {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
      dst_access_mask: vk::AccessFlags::HOST_READ,
      old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      // Optimal layouts can have different internal representations depending on what the driver
      // implemented, GENERAL must be used in order to interpret the image by the CPU
      new_layout: vk::ImageLayout::GENERAL,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      image: dst_image,
      subresource_range,
    };
    device.cmd_pipeline_barrier(
      self.copy_to_host,
      vk::PipelineStageFlags::TRANSFER,
      vk::PipelineStageFlags::HOST,
      vk::DependencyFlags::empty(),
      &[],
      &[],
      &[make_dst_host_accessible],
    );

    device
      .end_command_buffer(self.copy_to_host)
      .expect("Failed to finish recording command buffer");
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
