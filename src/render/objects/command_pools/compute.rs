use std::ptr;

use ash::vk;

use crate::{
  render::{
    objects::{device::QueueFamilies, ComputePipeline, DescriptorSets},
    push_constants::{ComputePushConstants, SpritePushConstants},
  },
  utility,
};

pub struct ComputeCommandBufferPool {
  pool: vk::CommandPool,
  pub buffer: vk::CommandBuffer,
}

impl ComputeCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Self {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_compute_index());

    let buffer = super::allocate_primary_command_buffers(device, pool, 1)[0];

    Self { pool, buffer }
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .expect("Failed to reset command pool");
  }

  pub unsafe fn record(
    &mut self,
    device: &ash::Device,
    index: usize,
    pipelines: &ComputePipeline,
    descriptor_sets: &DescriptorSets,
    player: &SpritePushConstants,
  ) {
    let cb = self.buffer;

    let command_buffer_begin_info = vk::CommandBufferBeginInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
      p_next: ptr::null(),
      p_inheritance_info: ptr::null(),
      flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
    };
    device
      .begin_command_buffer(cb, &command_buffer_begin_info)
      .expect("Failed to start recording command buffer");

    device.cmd_bind_descriptor_sets(
      cb,
      vk::PipelineBindPoint::COMPUTE,
      pipelines.layout,
      0,
      &[descriptor_sets.compute[index]],
      &[],
    );

    let compute_pc = ComputePushConstants {
      position: player.position,
    };

    device.cmd_push_constants(
      cb,
      pipelines.layout,
      vk::ShaderStageFlags::COMPUTE,
      0,
      utility::any_as_u8_slice(&compute_pc),
    );

    device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, pipelines.pipeline);

    device.cmd_dispatch(cb, 64, 1, 1);

    device
      .end_command_buffer(cb)
      .expect("Failed to finish recording command buffer")
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
