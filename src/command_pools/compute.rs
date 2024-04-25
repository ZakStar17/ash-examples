use std::{marker::PhantomData, ptr};

use ash::vk;

use crate::{
  descriptor_sets::DescriptorSets, device::QueueFamilies,
  device_destroyable::DeviceManuallyDestroyed, errors::OutOfMemoryError, pipeline::ComputePipeline,
  IMAGE_HEIGHT, IMAGE_WIDTH, SHADER_GROUP_SIZE_X, SHADER_GROUP_SIZE_Y,
};

use super::dependency_info;

pub struct ComputeCommandBufferPool {
  pool: vk::CommandPool,
  pub mandelbrot: vk::CommandBuffer,
}

impl ComputeCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Result<Self, vk::Result> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_compute_index())?;

    let mandelbrot = super::allocate_primary_command_buffers(device, pool, 1)?[0];

    Ok(Self { pool, mandelbrot })
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) -> Result<(), vk::Result> {
    device.reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
  }

  pub unsafe fn record_mandelbrot(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    pipeline: &ComputePipeline,
    descriptor_sets: &DescriptorSets,
    image: vk::Image,
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.mandelbrot;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    // image has 1 mip_level / 1 array layer
    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    let prepare_image = vk::ImageMemoryBarrier2 {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags2::NONE,
      dst_access_mask: vk::AccessFlags2::SHADER_WRITE,
      src_stage_mask: vk::PipelineStageFlags2::NONE,
      dst_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
      old_layout: vk::ImageLayout::UNDEFINED,
      new_layout: vk::ImageLayout::GENERAL,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      image,
      subresource_range,
      _marker: PhantomData,
    };
    device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[prepare_image]));

    // descriptor set should already have the image info written to it
    device.cmd_bind_descriptor_sets(
      cb,
      vk::PipelineBindPoint::COMPUTE,
      pipeline.layout,
      0,
      &[descriptor_sets.mandelbrot_image],
      &[],
    );
    device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, pipeline.pipeline);
    device.cmd_dispatch(
      cb,
      IMAGE_WIDTH / SHADER_GROUP_SIZE_X + 1,
      IMAGE_HEIGHT / SHADER_GROUP_SIZE_Y + 1,
      1,
    );

    if queue_families.get_compute_index() != queue_families.get_transfer_index() {
      let release = vk::ImageMemoryBarrier2 {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        dst_stage_mask: vk::PipelineStageFlags2::TRANSFER, // semaphore
        src_access_mask: vk::AccessFlags2::SHADER_WRITE,
        dst_access_mask: vk::AccessFlags2::NONE, // NONE for ownership release
        old_layout: vk::ImageLayout::GENERAL,
        new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        src_queue_family_index: queue_families.get_compute_index(),
        dst_queue_family_index: queue_families.get_transfer_index(),
        image,
        subresource_range,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[release]));
    } else {
      // if queues are equal just change image layout
      let change_layout = vk::ImageMemoryBarrier2 {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
        src_access_mask: vk::AccessFlags2::SHADER_WRITE,
        dst_access_mask: vk::AccessFlags2::TRANSFER_READ,
        old_layout: vk::ImageLayout::GENERAL,
        new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        image,
        subresource_range,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[change_layout]));
    }

    device.end_command_buffer(cb)?;

    Ok(())
  }
}

impl DeviceManuallyDestroyed for ComputeCommandBufferPool {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
