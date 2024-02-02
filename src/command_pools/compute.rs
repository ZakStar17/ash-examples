use std::ptr;

use ash::vk;

use crate::{
  descriptor_sets::DescriptorSets, device::QueueFamilies, pipeline::ComputePipeline, IMAGE_HEIGHT,
  IMAGE_WIDTH, SHADER_GROUP_SIZE_X, SHADER_GROUP_SIZE_Y,
};

pub struct ComputeCommandBufferPool {
  pool: vk::CommandPool,
  // executes a compute shader that writes to a storage image
  pub storage_image: vk::CommandBuffer,
}

impl ComputeCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Self {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_compute_index());

    let storage_image = super::allocate_primary_command_buffers(device, pool, 1)[0];

    Self {
      pool,
      storage_image,
    }
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .expect("Failed to reset command pool");
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }

  pub unsafe fn record_mandelbrot(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    pipeline: &ComputePipeline,
    descriptor_sets: &DescriptorSets,
    image: vk::Image,
  ) {
    let cb = self.storage_image;
    let begin_info = vk::CommandBufferBeginInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
      p_next: ptr::null(),
      flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
      p_inheritance_info: ptr::null(),
    };
    device
      .begin_command_buffer(cb, &begin_info)
      .expect("Failed to begin recording command buffer");

    // image has 1 mip_level / 1 array layer
    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    let shader_write_layout = vk::ImageMemoryBarrier {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags::NONE,
      dst_access_mask: vk::AccessFlags::SHADER_WRITE,
      old_layout: vk::ImageLayout::UNDEFINED,
      // image layout is required to be GENERAL in order to be used as storage in a shader
      new_layout: vk::ImageLayout::GENERAL,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      image,
      subresource_range,
    };
    device.cmd_pipeline_barrier(
      cb,
      // this operation doesn't have to wait for anything
      vk::PipelineStageFlags::NONE,
      // however it should finish before the compute shader
      vk::PipelineStageFlags::COMPUTE_SHADER,
      vk::DependencyFlags::empty(),
      &[],
      &[],
      &[shader_write_layout],
    );

    // descriptor set should already have the image info written to it
    device.cmd_bind_descriptor_sets(
      cb,
      vk::PipelineBindPoint::COMPUTE,
      pipeline.layout,
      0,
      &[descriptor_sets.pool.mandelbrot],
      &[],
    );
    device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, pipeline.pipeline);
    device.cmd_dispatch(
      cb,
      IMAGE_WIDTH / SHADER_GROUP_SIZE_X + 1,
      IMAGE_HEIGHT / SHADER_GROUP_SIZE_Y + 1,
      1,
    );

    // Release image to transfer queue family and change image layout at the same time
    // Even though the layout transition operation is submitted twice, it only executes once in
    // between queue ownership transfer
    // https://docs.vulkan.org/spec/latest/chapters/synchronization.html#synchronization-queue-transfers
    let release = vk::ImageMemoryBarrier {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags::SHADER_WRITE,
      dst_access_mask: vk::AccessFlags::NONE, // should be NONE for ownership release
      old_layout: vk::ImageLayout::GENERAL,
      new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
      src_queue_family_index: queue_families.get_compute_index(),
      dst_queue_family_index: queue_families.get_transfer_index(),
      image,
      subresource_range,
    };
    device.cmd_pipeline_barrier(
      cb,
      // wait for the shader to complete before transferring
      vk::PipelineStageFlags::COMPUTE_SHADER,
      vk::PipelineStageFlags::TRANSFER,
      vk::DependencyFlags::empty(),
      &[],
      &[],
      &[release],
    );

    device
      .end_command_buffer(cb)
      .expect("Failed to finish recording command buffer");
  }
}
