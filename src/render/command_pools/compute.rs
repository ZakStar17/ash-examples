use std::{marker::PhantomData, mem::size_of, ops::BitOr, ptr};

use ash::vk;

use crate::{
  render::{
    data::compute::{Bullet, ComputeData, ComputeDataUpdate, ComputePushConstants},
    descriptor_sets::DescriptorPool,
    device_destroyable::DeviceManuallyDestroyed,
    errors::OutOfMemoryError,
    initialization::device::QueueFamilies,
    pipelines::ComputePipelines,
    FRAMES_IN_FLIGHT,
  },
  utility,
};

use super::dependency_info;

pub struct ComputeCommandPool {
  pool: vk::CommandPool,
  pub instance: vk::CommandBuffer,
}

impl ComputeCommandPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Result<Self, vk::Result> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_compute_index())?;

    let instance = super::allocate_primary_command_buffers(device, pool, 1)?[0];

    Ok(Self { pool, instance })
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) -> Result<(), OutOfMemoryError> {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .map_err(|err| err.into())
  }

  pub unsafe fn record(
    &mut self,
    frame_i: usize,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    pipelines: &ComputePipelines,
    descriptor_pool: &DescriptorPool,

    data: &ComputeData, // buffers
    update_data: ComputeDataUpdate,
    player_pos: [f32; 2],
    // returns effective instance size
  ) -> Result<u64, OutOfMemoryError> {
    let cb = self.instance;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    // before shader for now
    if let Some((right_region, left_opt)) = update_data.copy_new_random_values {
      let mut copy_barrier = vk::BufferMemoryBarrier2 {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::UNIFORM_READ.bitor(vk::AccessFlags2::TRANSFER_WRITE),
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER
          .bitor(vk::PipelineStageFlags2::COPY),
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        buffer: data.device.random_values[frame_i],
        offset: 0,            // set later
        size: vk::WHOLE_SIZE, // set later
        _marker: PhantomData,
      };

      let right_offset = (right_region.start * size_of::<f32>()) as u64;
      let right_size = ((right_region.end - right_region.start) * size_of::<f32>()) as u64;
      let right_copy = vk::BufferCopy {
        src_offset: right_offset,
        dst_offset: right_offset, // same sizes
        size: right_size,
      };

      if let Some(left_region) = left_opt {
        // two ranges should occur rarely

        let left_offset = 0; // RandomBufferUsedArea::raw_ranges
        let left_size = (left_region.end * size_of::<f32>()) as u64;
        let left_copy = vk::BufferCopy {
          src_offset: left_offset,
          dst_offset: left_offset, // same sizes
          size: left_size,
        };

        device.cmd_copy_buffer(
          cb,
          *data.host.random_values[frame_i],
          data.device.random_values[frame_i],
          &[right_copy, left_copy],
        );

        // affect whole buffer as to not create more than one memory barrier
        // with the same parameters
        copy_barrier.offset = 0;
        copy_barrier.size = vk::WHOLE_SIZE;
      } else {
        device.cmd_copy_buffer(
          cb,
          *data.host.random_values[frame_i],
          data.device.random_values[frame_i],
          &[right_copy],
        );
        copy_barrier.offset = right_copy.dst_offset;
        copy_barrier.size = right_copy.size;
      }

      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[copy_barrier], &[]));
    }

    // read only storage buffer from previous dispatch (old)
    let src = data.device.instance_compute[(frame_i + 1) % FRAMES_IN_FLIGHT];
    // write only storage buffer to update
    let dst = data.device.instance_compute[frame_i];
    // host input/output
    let host_inp_out = data.host.compute_host_io[frame_i].buffer;
    let graphics = data.device.instance_graphics[frame_i];

    let affected_instance_size =
      (update_data.target_bullet_count as usize * size_of::<Bullet>()) as u64;

    let push_constants = ComputePushConstants {
      player_pos,
      bullet_count: update_data.bullet_count,
      target_bullet_count: update_data.target_bullet_count,
      random_uniform_reserved_index: update_data.random_uniform_reserved_index,
    };

    if update_data.compute_io_updated {
      // sync host writes
      let host_input = vk::BufferMemoryBarrier2 {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::HOST_WRITE,
        dst_access_mask: vk::AccessFlags2::SHADER_READ.bitor(vk::AccessFlags2::SHADER_WRITE),
        src_stage_mask: vk::PipelineStageFlags2::HOST,
        dst_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        buffer: host_inp_out,
        offset: 0,
        size: vk::WHOLE_SIZE,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[host_input], &[]));
    }

    // shader dispatch
    {
      device.cmd_bind_descriptor_sets(
        cb,
        vk::PipelineBindPoint::COMPUTE,
        pipelines.layout,
        0,
        &[descriptor_pool.compute_sets[frame_i]],
        &[],
      );
      device.cmd_push_constants(
        cb,
        pipelines.layout,
        vk::ShaderStageFlags::COMPUTE,
        0,
        utility::any_as_u8_slice(&push_constants),
      );
      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, pipelines.instance);

      // todo: local shader size constant
      let group_count = push_constants.target_bullet_count / 16 + 1;
      device.cmd_dispatch(cb, group_count, 1, 1);
    }

    // make shader results visible to subsequent operations
    {
      let src_next_write_exec_barrier = vk::BufferMemoryBarrier2 {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::NONE,
        dst_access_mask: vk::AccessFlags2::SHADER_WRITE,
        src_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        dst_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        buffer: src,
        offset: 0,
        size: affected_instance_size,
        _marker: PhantomData,
      };
      let dst_next_copy_and_shader_barriers = vk::BufferMemoryBarrier2 {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::SHADER_WRITE,
        dst_access_mask: vk::AccessFlags2::SHADER_READ.bitor(vk::AccessFlags2::TRANSFER_READ),
        src_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        dst_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER
          .bitor(vk::PipelineStageFlags2::COPY),
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        buffer: dst,
        offset: 0,
        size: affected_instance_size,
        _marker: PhantomData,
      };
      let host_out = vk::BufferMemoryBarrier2 {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::SHADER_WRITE,
        // no shader read because of previous "host_input" barrier
        dst_access_mask: vk::AccessFlags2::HOST_READ,
        src_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        dst_stage_mask: vk::PipelineStageFlags2::HOST,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        buffer: host_inp_out,
        offset: 0,
        size: vk::WHOLE_SIZE,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(
        cb,
        &dependency_info(
          &[],
          &[
            src_next_write_exec_barrier,
            dst_next_copy_and_shader_barriers,
            host_out,
          ],
          &[],
        ),
      );
    }

    {
      let effective_region = vk::BufferCopy {
        src_offset: 0,
        dst_offset: 0,
        size: affected_instance_size,
      };
      device.cmd_copy_buffer(cb, dst, graphics, &[effective_region]);

      let mut release_graphics = vk::BufferMemoryBarrier2 {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::UNIFORM_READ.bitor(vk::AccessFlags2::TRANSFER_WRITE),
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::VERTEX_INPUT.bitor(vk::PipelineStageFlags2::COPY),
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        buffer: graphics,
        offset: 0,
        size: affected_instance_size,
        _marker: PhantomData,
      };
      if queue_families.get_graphics_index() != queue_families.get_compute_index() {
        release_graphics.dst_access_mask = vk::AccessFlags2::NONE;
        release_graphics.src_queue_family_index = queue_families.get_compute_index();
        release_graphics.dst_queue_family_index = queue_families.get_graphics_index();
      }
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[release_graphics], &[]));
    }

    device.end_command_buffer(cb)?;
    Ok(affected_instance_size)
  }
}

impl DeviceManuallyDestroyed for ComputeCommandPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.pool.destroy_self(device);
  }
}
