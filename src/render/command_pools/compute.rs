use std::{mem::size_of, ops::BitOr, ptr};

use ash::vk;

use crate::{
  render::{
    compute_data::{ComputePushConstants, Projectile},
    initialization::QueueFamilies,
    pipelines::ComputePipelines,
  },
  utility,
};

use super::dependency_info;

pub struct ComputeCommandPool {
  pool: vk::CommandPool,
  pub buffer: vk::CommandBuffer,
}

#[derive(Debug)]
pub struct AddNewProjectiles {
  pub buffer: vk::Buffer,
  pub buffer_size: u64,
  pub bullet_count: usize,
}

#[derive(Debug)]
pub struct ExecuteShader {
  pub push_data: ComputePushConstants,
}

#[derive(Debug)]
pub struct ComputeRecordBufferData {
  pub output: vk::Buffer,
  pub instance_read: vk::Buffer,
  pub instance_write: vk::Buffer,
  pub instance_graphics: vk::Buffer,
  pub existing_projectiles_count: usize,
  pub add_projectiles: Option<AddNewProjectiles>,
  pub execute_shader: Option<ExecuteShader>,
}

impl ComputeCommandPool {
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
    pipelines: &ComputePipelines,
    descriptor_set: vk::DescriptorSet,
    data: ComputeRecordBufferData,
  ) {
    let cb = self.buffer;

    // println!("Recording {:#?}", data);

    let command_buffer_begin_info = vk::CommandBufferBeginInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
      p_next: ptr::null(),
      p_inheritance_info: ptr::null(),
      flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
    };
    device
      .begin_command_buffer(cb, &command_buffer_begin_info)
      .expect("Failed to start recording command buffer");

    let base_barrier = vk::BufferMemoryBarrier2 {
      s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags2::NONE,
      dst_access_mask: vk::AccessFlags2::NONE,
      src_stage_mask: vk::PipelineStageFlags2::NONE,
      dst_stage_mask: vk::PipelineStageFlags2::NONE,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      buffer: vk::Buffer::null(),
      offset: 0,
      size: vk::WHOLE_SIZE,
    };

    let existing_projectiles_size =
      (size_of::<Projectile>() * data.existing_projectiles_count) as u64;

    if let Some(shader_data) = data.execute_shader {
      // clear output buffer
      device.cmd_fill_buffer(cb, data.output, 0, vk::WHOLE_SIZE, 0);
      let flush = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::SHADER_STORAGE_READ
          .bitor(vk::AccessFlags2::SHADER_STORAGE_WRITE),
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        buffer: data.output,
        ..base_barrier
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[flush], &[]));

      let eh: vk::BufferMemoryBarrier2 = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::SHADER_STORAGE_WRITE,
        dst_access_mask: vk::AccessFlags2::SHADER_STORAGE_READ,
        src_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        dst_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        buffer: data.instance_read,
        ..base_barrier
      };
      device.cmd_pipeline_barrier2(
        cb,
        &dependency_info(&[], &[eh], &[]),
      );
      device.cmd_bind_descriptor_sets(
        cb,
        vk::PipelineBindPoint::COMPUTE,
        pipelines.layout,
        0,
        &[descriptor_set],
        &[],
      );
      device.cmd_push_constants(
        cb,
        pipelines.layout,
        vk::ShaderStageFlags::COMPUTE,
        0,
        utility::any_as_u8_slice(&shader_data.push_data),
      );
      device.cmd_bind_pipeline(
        cb,
        vk::PipelineBindPoint::COMPUTE,
        pipelines.compute_instances,
      );

      let group_count = data.existing_projectiles_count / 8 + 1;
      device.cmd_dispatch(cb, group_count as u32, 1, 1);

      // let write_flush = vk::BufferMemoryBarrier2 {
      //   src_access_mask: vk::AccessFlags2::SHADER_STORAGE_WRITE,
      //   dst_access_mask: vk::AccessFlags2::TRANSFER_READ.bitor(vk::AccessFlags2::SHADER_STORAGE_READ),
      //   src_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
      //   dst_stage_mask: vk::PipelineStageFlags2::COPY.bitor(vk::PipelineStageFlags2::COMPUTE_SHADER),
      //   buffer: data.instance_write,
      //   size: existing_projectiles_size,
      //   ..base_barrier
      // };
      let output_flush = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::SHADER_STORAGE_WRITE,
        dst_access_mask: vk::AccessFlags2::HOST_READ,
        src_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        dst_stage_mask: vk::PipelineStageFlags2::HOST,
        buffer: data.output,
        ..base_barrier
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[output_flush], &[]));
    }

    if let Some(add_new) = data.add_projectiles.as_ref() {
      let flush_host = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::HOST_WRITE,
        dst_access_mask: vk::AccessFlags2::TRANSFER_READ,
        src_stage_mask: vk::PipelineStageFlags2::HOST,
        dst_stage_mask: vk::PipelineStageFlags2::COPY,
        buffer: add_new.buffer,
        size: add_new.buffer_size,
        ..base_barrier
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[flush_host], &[]));

      let new_bullets_region = vk::BufferCopy {
        src_offset: 0,
        dst_offset: existing_projectiles_size,
        size: add_new.buffer_size,
      };
      // device.cmd_copy_buffer(
      //   cb,
      //   add_new.buffer,
      //   data.instance_write,
      //   &[new_bullets_region],
      // );
      device.cmd_copy_buffer(
        cb,
        add_new.buffer,
        data.instance_graphics,
        &[new_bullets_region],
      );
    }

    // if existing_projectiles_size > 0 {
    //   let graphics_region = vk::BufferCopy {
    //     src_offset: 0,
    //     dst_offset: 0,
    //     size: existing_projectiles_size,
    //   };
    //   device.cmd_copy_buffer(
    //     cb,
    //     data.instance_write,
    //     data.instance_graphics,
    //     &[graphics_region],
    //   );
    // }

    {
      let release_graphics = vk::BufferMemoryBarrier2 {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::NONE, // ownership release
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
        src_queue_family_index: queue_families.get_compute_index(),
        dst_queue_family_index: queue_families.get_graphics_index(),
        buffer: data.instance_graphics,
        offset: 0,
        size: vk::WHOLE_SIZE,
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[release_graphics], &[]));
    }

    device
      .end_command_buffer(cb)
      .expect("Failed to finish recording command buffer")
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
