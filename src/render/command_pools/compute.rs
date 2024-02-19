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

pub struct ComputeCommandPool {
  pool: vk::CommandPool,
  pub buffer: vk::CommandBuffer,
}

pub struct AddNewProjectiles {
  pub buffer: vk::Buffer,
  pub count: usize,
}

pub struct ExecuteShader {
  pub push_data: ComputePushConstants,
}

pub struct ComputeRecordBufferData {
  pub output: vk::Buffer,
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

    let command_buffer_begin_info = vk::CommandBufferBeginInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
      p_next: ptr::null(),
      p_inheritance_info: ptr::null(),
      flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
    };
    device
      .begin_command_buffer(cb, &command_buffer_begin_info)
      .expect("Failed to start recording command buffer");

    let base_memory_barrier = vk::BufferMemoryBarrier {
      s_type: vk::StructureType::BUFFER_MEMORY_BARRIER,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags::empty(),
      dst_access_mask: vk::AccessFlags::empty(),
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      buffer: vk::Buffer::null(),
      offset: 0,
      size: vk::WHOLE_SIZE,
    };

    // clear output buffer before shader runs
    device.cmd_fill_buffer(cb, data.output, 0, vk::WHOLE_SIZE, 0);

    let existing_projectiles_size =
      (size_of::<Projectile>() * data.existing_projectiles_count) as u64;

    if let Some(add_new) = data.add_projectiles.as_ref() {
      let flush_new_projectiles = vk::BufferMemoryBarrier {
        src_access_mask: vk::AccessFlags::HOST_WRITE,
        dst_access_mask: vk::AccessFlags::TRANSFER_READ,
        buffer: add_new.buffer,
        ..base_memory_barrier
      };
      device.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::HOST,
        vk::PipelineStageFlags::TRANSFER,
        vk::DependencyFlags::empty(),
        &[],
        &[flush_new_projectiles],
        &[],
      );

      let region = vk::BufferCopy {
        src_offset: 0,
        dst_offset: existing_projectiles_size,
        size: (size_of::<Projectile>() * add_new.count) as u64,
      };
      device.cmd_copy_buffer(cb, add_new.buffer, data.instance_write, &[region]);
    }

    if let Some(shader_data) = data.execute_shader {
      {
        let wait_for_output_buffer = vk::BufferMemoryBarrier {
          src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
          dst_access_mask: vk::AccessFlags::SHADER_READ.bitor(vk::AccessFlags::SHADER_WRITE),
          buffer: data.output,
          ..base_memory_barrier
        };
        device.cmd_pipeline_barrier(
          cb,
          vk::PipelineStageFlags::TRANSFER,
          vk::PipelineStageFlags::COMPUTE_SHADER,
          vk::DependencyFlags::empty(),
          &[],
          &[wait_for_output_buffer],
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
      }

      {
        let instance_write_wait_shader_before_transfer = vk::BufferMemoryBarrier {
          src_access_mask: vk::AccessFlags::SHADER_WRITE,
          dst_access_mask: vk::AccessFlags::TRANSFER_READ,
          buffer: data.instance_write,
          size: existing_projectiles_size,
          ..base_memory_barrier
        };
        device.cmd_pipeline_barrier(
          cb,
          vk::PipelineStageFlags::COMPUTE_SHADER,
          vk::PipelineStageFlags::TRANSFER,
          vk::DependencyFlags::empty(),
          &[],
          &[instance_write_wait_shader_before_transfer],
          &[],
        );
      }

      {
        let flush_output_after_shader = vk::BufferMemoryBarrier {
          src_access_mask: vk::AccessFlags::SHADER_WRITE,
          dst_access_mask: vk::AccessFlags::HOST_READ,
          buffer: data.output,
          ..base_memory_barrier
        };
        device.cmd_pipeline_barrier(
          cb,
          vk::PipelineStageFlags::COMPUTE_SHADER,
          vk::PipelineStageFlags::HOST,
          vk::DependencyFlags::empty(),
          &[],
          &[flush_output_after_shader],
          &[],
        );
      }
    } else {
      let flush_output_after_clear = vk::BufferMemoryBarrier {
        src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags::HOST_READ,
        buffer: data.output,
        ..base_memory_barrier
      };
      device.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::HOST,
        vk::DependencyFlags::empty(),
        &[],
        &[flush_output_after_clear],
        &[],
      );
    }

    {
      let region = if let Some(add_new) = data.add_projectiles.as_ref() {
        let new_projectiles_size = (size_of::<Projectile>() * add_new.count) as u64;

        {
          let wait_transfer = vk::BufferMemoryBarrier {
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
            dst_access_mask: vk::AccessFlags::TRANSFER_READ,
            buffer: data.instance_write,
            offset: existing_projectiles_size,
            size: new_projectiles_size,
            ..base_memory_barrier
          };
          device.cmd_pipeline_barrier(
            cb,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[wait_transfer],
            &[],
          );
        }

        vk::BufferCopy {
          src_offset: 0,
          dst_offset: 0,
          size: existing_projectiles_size + new_projectiles_size,
        }
      } else {
        vk::BufferCopy {
          src_offset: 0,
          dst_offset: 0,
          size: existing_projectiles_size,
        }
      };
      device.cmd_copy_buffer(cb, data.instance_write, data.instance_graphics, &[region]);

      let release_graphics_buffer = vk::BufferMemoryBarrier {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags::NONE, // should be NONE for ownership release
        src_queue_family_index: queue_families.get_compute_index(),
        dst_queue_family_index: queue_families.get_graphics_index(),
        buffer: data.instance_graphics,
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
