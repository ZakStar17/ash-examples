use std::{mem::size_of, ops::BitOr, ptr};

use ash::vk;

use crate::{
  render::{
    compute_data::{ComputeData, Projectile},
    objects::{device::QueueFamilies, ComputePipeline, DescriptorSets},
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
    queue_families: &QueueFamilies,
    index: usize,
    pipelines: &ComputePipeline,
    descriptor_sets: &DescriptorSets,
    compute_data: &ComputeData,
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

    // clear output buffer before shader runs
    device.cmd_fill_buffer(cb, compute_data.shader_output[index], 0, vk::WHOLE_SIZE, 0);
    {
      let shader_output_clear_barrier = vk::BufferMemoryBarrier {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags::SHADER_READ.bitor(vk::AccessFlags::SHADER_WRITE),
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        buffer: compute_data.shader_output[index],
        offset: 0,
        size: vk::WHOLE_SIZE,
      };
      device.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::COMPUTE_SHADER,
        vk::DependencyFlags::empty(),
        &[],
        &[shader_output_clear_barrier],
        &[],
      );
    }

    // shader invocation
    {
      device.cmd_bind_descriptor_sets(
        cb,
        vk::PipelineBindPoint::COMPUTE,
        pipelines.layout,
        0,
        &[descriptor_sets.compute_sets[index]],
        &[],
      );
      device.cmd_push_constants(
        cb,
        pipelines.layout,
        vk::ShaderStageFlags::COMPUTE,
        0,
        utility::any_as_u8_slice(&compute_data.constants),
      );
      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, pipelines.pipeline);

      // one invocation for each existing projectile + 1 to add new projectiles
      let group_count: u32 = (compute_data.constants.cur_projectile_count + 1) / 8 + 1;
      device.cmd_dispatch(cb, group_count, 1, 1);
    }

    // flush shader outputs
    {
      let instate_compute_write = vk::BufferMemoryBarrier {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags::SHADER_WRITE,
        dst_access_mask: vk::AccessFlags::TRANSFER_READ,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        buffer: compute_data.instance_compute[index], // buffer that had been written to
        offset: 0,
        size: vk::WHOLE_SIZE,
      };
      device.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::COMPUTE_SHADER,
        vk::PipelineStageFlags::TRANSFER,
        vk::DependencyFlags::empty(),
        &[],
        &[instate_compute_write],
        &[],
      );

      let shader_output = vk::BufferMemoryBarrier {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags::SHADER_WRITE,
        dst_access_mask: vk::AccessFlags::HOST_READ, // flush to host
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        buffer: compute_data.shader_output[index],
        offset: 0,
        size: vk::WHOLE_SIZE,
      };
      device.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::COMPUTE_SHADER,
        vk::PipelineStageFlags::HOST,
        vk::DependencyFlags::empty(),
        &[],
        &[shader_output],
        &[],
      );
    }

    let region = vk::BufferCopy {
      src_offset: 0,
      dst_offset: 0,
      size: (compute_data.max_valid_projectile_count() * size_of::<Projectile>()) as u64,
    };
    // instance graphics belongs to a different queue but its contents need not to be preserved
    device.cmd_copy_buffer(
      cb,
      compute_data.instance_compute[index],
      compute_data.instance_graphics[index],
      &[region],
    );

    {
      let release_graphics_buffer = vk::BufferMemoryBarrier {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags::NONE, // should be NONE for ownership release
        src_queue_family_index: queue_families.get_compute_index(),
        dst_queue_family_index: queue_families.get_graphics_index(),
        buffer: compute_data.instance_graphics[index],
        offset: 0,
        size: vk::WHOLE_SIZE,
      };
      device.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::TRANSFER,
        vk::DependencyFlags::empty(),
        &[],
        &[release_graphics_buffer],
        &[],
      );
    }

    device
      .end_command_buffer(cb)
      .expect("Failed to finish recording command buffer")
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
