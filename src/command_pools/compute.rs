use std::ptr;

use ash::vk;

use crate::{device::QueueFamilies, IMAGE_COLOR};

pub struct ComputeCommandBufferPool {
  pool: vk::CommandPool,
  pub clear_img: vk::CommandBuffer,
}

impl ComputeCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Self {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_compute_index());

    let clear_img = super::allocate_primary_command_buffers(device, pool, 1)[0];

    Self { pool, clear_img }
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .expect("Failed to reset command pool");
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }

  pub unsafe fn record_clear_img(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    image: vk::Image,
  ) {
    let begin_info = vk::CommandBufferBeginInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
      p_next: ptr::null(),
      flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
      p_inheritance_info: ptr::null(),
    };
    device
      .begin_command_buffer(self.clear_img, &begin_info)
      .expect("Failed to begin recording command buffer");

    // image has 1 mip_level / 1 array layer
    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    // change image layout and access to transfer write destination
    let transfer_dst_layout = vk::ImageMemoryBarrier {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags::empty(),
      dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
      old_layout: vk::ImageLayout::UNDEFINED,
      new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      image,
      subresource_range,
    };
    device.cmd_pipeline_barrier(
      self.clear_img,
      vk::PipelineStageFlags::TRANSFER,
      vk::PipelineStageFlags::TRANSFER,
      vk::DependencyFlags::empty(),
      &[],
      &[],
      &[transfer_dst_layout],
    );

    device.cmd_clear_color_image(
      self.clear_img,
      image,
      vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      &IMAGE_COLOR,
      &[subresource_range],
    );

    // Release image to transfer queue family and change image layout at the same time
    // Even though the layout transition operation is submitted twice, it only executes once in
    // between queue ownership transfer
    // https://docs.vulkan.org/spec/latest/chapters/synchronization.html#synchronization-queue-transfers
    let release = vk::ImageMemoryBarrier {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      // this operation needs to know access flags before the transition
      src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
      dst_access_mask: vk::AccessFlags::NONE, // should be NONE for ownership release
      old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
      src_queue_family_index: queue_families.get_compute_index(),
      dst_queue_family_index: queue_families.get_transfer_index(),
      image,
      subresource_range,
    };
    device.cmd_pipeline_barrier(
      self.clear_img,
      // If you are using cmd_pipeline_barrier2, you can wait specifically for
      // vk::PipelineStageFlags2::CLEAR
      // wait for all TRANSFER operations
      vk::PipelineStageFlags::TRANSFER,
      // before continuing any subsequent transfer operations
      vk::PipelineStageFlags::TRANSFER,
      vk::DependencyFlags::empty(),
      &[],
      &[],
      &[release],
    );

    device
      .end_command_buffer(self.clear_img)
      .expect("Failed to finish recording command buffer");
  }
}
