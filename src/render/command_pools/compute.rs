use std::{marker::PhantomData, mem::size_of, ops::BitOr, ptr};

use ash::vk;

use crate::{
  render::{
    data::compute::{Bullet, ComputeData, ComputePushConstants},
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

#[derive(Debug)]
pub struct AddNewBullets {
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
  pub existing_bullets_count: usize,
  pub add_bullets: Option<AddNewBullets>,
  pub execute_shader: Option<ExecuteShader>,
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

    data: ComputeData,
    push_constants: ComputePushConstants,

    refresh_random_buffer: Option<usize>,
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.instance;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    if let Some(count) = refresh_random_buffer {
      let affected_new_random_values_size = (count * size_of::<f32>()) as u64;
      let region = vk::BufferCopy {
        src_offset: 0,
        dst_offset: 0,
        size: affected_new_random_values_size,
      };
      device.cmd_copy_buffer(
        cb,
        *data.host.staging_random_values[frame_i],
        data.device.device_random_values[frame_i],
        &[region],
      );

      // flush for compute and next copy writes
      let flush = vk::BufferMemoryBarrier2 {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::UNIFORM_READ.bitor(vk::AccessFlags2::TRANSFER_WRITE),
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER
          .bitor(vk::PipelineStageFlags2::COPY),
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        buffer: data.device.device_random_values[frame_i],
        offset: 0,
        size: affected_new_random_values_size,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[flush], &[]));
    }

    // read only storage buffer from previous dispatch (old)
    let src = data.device.instance_compute[(frame_i + 1) % FRAMES_IN_FLIGHT];
    // write only storage buffer to update
    let dst = data.device.instance_compute[frame_i];
    // host input/output
    let host_inp_out = data.host.storage_output[frame_i].buffer;
    let graphics = data.device.instance_graphics[frame_i];

    let affected_instance_size = (data.target_bullet_count * size_of::<Bullet>()) as u64;

    // sync host writes
    {
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
    }

    device.end_command_buffer(cb)?;
    Ok(())
  }
}

impl DeviceManuallyDestroyed for ComputeCommandPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.pool.destroy_self(device);
  }
}
