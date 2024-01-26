use std::ptr;

use ash::vk;

use crate::physical_device::QueueFamilies;

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

fn create_primary_command_buffers(
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
  // may fail if out of memory
  unsafe {
    device
      .allocate_command_buffers(&allocate_info)
      .expect("Failed to allocate command buffers")
  }
}

pub struct ComputeCommandBufferPool {
  pool: vk::CommandPool,
  pub clear_img: vk::CommandBuffer,
}

impl ComputeCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Self {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = create_command_pool(device, flags, queue_families.get_compute_index());

    let clear_img = create_primary_command_buffers(device, pool, 1)[0];

    Self { pool, clear_img }
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) {
    device.reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty()).expect("Failed to reset command pool");
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }

  pub unsafe fn record_clear_img(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    local_image: vk::Image,
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

    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    let image_layout_dst_barrier = vk::ImageMemoryBarrier {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags::empty(),
      dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
      old_layout: vk::ImageLayout::UNDEFINED,
      new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      image: local_image,
      subresource_range,
    };
    device.cmd_pipeline_barrier(
      self.clear_img,
      vk::PipelineStageFlags::TRANSFER, // really just waiting for nothing
      // if something were to be executed previously, then it should complete before transfer
      vk::PipelineStageFlags::TRANSFER,
      vk::DependencyFlags::empty(),
      &[],
      &[],
      &[image_layout_dst_barrier],
    );

    let clear_color = vk::ClearColorValue {
      float32: [0.0, 0.0, 0.0, 0.0],
    };
    device.cmd_clear_color_image(
      self.clear_img,
      local_image,
      vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      &clear_color,
      &[subresource_range],
    );

    let image_pass_to_transfer_queue_barrier = vk::ImageMemoryBarrier {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
      dst_access_mask: vk::AccessFlags::TRANSFER_READ,
      old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
      src_queue_family_index: queue_families.get_compute_index(),
      dst_queue_family_index: queue_families.get_transfer_index(),
      image: local_image,
      subresource_range,
    };
    device.cmd_pipeline_barrier(
      self.clear_img,
      // waiting for clear color operation
      vk::PipelineStageFlags::TRANSFER,
      vk::PipelineStageFlags::TRANSFER,
      vk::DependencyFlags::empty(),
      &[],
      &[],
      &[image_pass_to_transfer_queue_barrier],
    );

    device
      .end_command_buffer(self.clear_img)
      .expect("Failed to finish recording command buffer");
  }
}
