use std::{mem::offset_of, ops::BitOr, ptr::NonNull};

use ash::vk;
use rand::rngs::ThreadRng;

use crate::{
  render::{
    allocator::allocate_and_bind_memory,
    create_objs::create_buffer,
    device_destroyable::{create_destroyable_array, destroy, DeviceManuallyDestroyed},
    errors::AllocationError,
    initialization::device::{Device, PhysicalDevice},
    FRAMES_IN_FLIGHT,
  },
  utility::{self, OnErr},
};

pub const MAX_NEW_PROJECTILES_PER_FRAME: usize = 1024;

// all data passed to the shader follows std430 layout rules
// https://www.oreilly.com/library/view/opengl-programming-guide/9780132748445/app09lev1sec3.html

// size and alignment: 4
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Bullet {
  pos: [f32; 2],
  vel: [f32; 2],
}

// impl instance vertex for Bullet
impl Bullet {
  const ATTRIBUTE_SIZE: usize = 2;

  pub const fn get_binding_description(binding: u32) -> vk::VertexInputBindingDescription {
    vk::VertexInputBindingDescription {
      binding,
      stride: size_of::<Self>() as u32,
      input_rate: vk::VertexInputRate::INSTANCE,
    }
  }

  pub const fn get_attribute_descriptions(
    offset: u32,
    binding: u32,
  ) -> [vk::VertexInputAttributeDescription; Self::ATTRIBUTE_SIZE] {
    [
      vk::VertexInputAttributeDescription {
        location: offset,
        binding,
        format: vk::Format::R32G32_SFLOAT,
        offset: offset_of!(Self, pos) as u32,
      },
      vk::VertexInputAttributeDescription {
        location: offset + 1,
        binding,
        format: vk::Format::R32G32_SFLOAT,
        offset: offset_of!(Self, vel) as u32,
      },
    ]
  }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct ComputePushConstants {
  pub player_pos: [f32; 2], // size: 2
  pub delta_time: f32,      // size: 1

  pub bullet_count: u32, // size: 4
}

// host accessible data after shader dispatch
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ComputeOutput {
  // 1 if colliding with bullet, 0 otherwise
  colliding: u32,
  // number of random values remaining for use in ComputeData::device_random_values
  random_values_left: u32,
}

// buffer and its mapped ptr
#[derive(Debug)]
pub struct MappedHostBuffer<T> {
  pub buffer: vk::Buffer,
  pub data_ptr: NonNull<T>,
}

#[derive(Debug)]
pub struct HostComputeData {
  pub memory: vk::DeviceMemory,
  // host storage buffer containing <ComputeOutput> data
  pub storage_output: [MappedHostBuffer<ComputeOutput>; FRAMES_IN_FLIGHT],
  // random values before being copied to device memory
  pub staging_random_values: MappedHostBuffer<f32>,
}

impl HostComputeData {
  const STAGING_RANDOM_VALUES_COUNT: u64 = 2u64.pow(16);
  const HOST_MEMORY_PRIORITY: f32 = 0.3;
}

#[derive(Debug)]
pub struct DeviceComputeData {
  pub memory: vk::DeviceMemory,
  pub instance_capacity: u64,
  // instance data that always belongs to the compute family
  pub instance_compute: [vk::Buffer; FRAMES_IN_FLIGHT],
  // instance data that gets copied from instance_compute to be used in graphics
  pub instance_graphics: [vk::Buffer; FRAMES_IN_FLIGHT],
  // cpus generate (pseudo)random values more easily than gpus, so these are generated in
  // cpu memory and then copied to the gpu
  pub device_random_values: [vk::Buffer; FRAMES_IN_FLIGHT],
}

#[derive(Debug)]
pub struct ComputeData {
  host: HostComputeData,
  device: DeviceComputeData,

  target_bullet_count: usize,
  cur_bullet_count: usize,
  rng: ThreadRng,
}

#[derive(Debug)]
struct StagingMemoryAllocation {
  pub memory: vk::DeviceMemory,
  pub memory_type: u32,
  pub storage_output_offsets: [u64; FRAMES_IN_FLIGHT],
  pub staging_random_values_offset: u64,
}

impl HostComputeData {
  pub fn create_and_allocate(
    device: &Device,
    physical_device: &PhysicalDevice,
  ) -> Result<Self, AllocationError> {
    let storage_output_buffers = create_destroyable_array!(
      device,
      create_buffer(
        device,
        size_of::<ComputeOutput>() as u64,
        vk::BufferUsageFlags::TRANSFER_DST.bitor(vk::BufferUsageFlags::STORAGE_BUFFER)
      ),
      FRAMES_IN_FLIGHT
    )?;

    let staging_random_values_buffer = create_buffer(
      device,
      size_of::<f32>() as u64 * Self::STAGING_RANDOM_VALUES_COUNT,
      vk::BufferUsageFlags::TRANSFER_SRC,
    )
    .on_err(|_| unsafe { storage_output_buffers.destroy_self(device) })?;

    let alloc = Self::allocate_memory(
      device,
      physical_device,
      storage_output_buffers,
      staging_random_values_buffer,
    )
    .on_err(|_| unsafe {
      destroy!(device => storage_output_buffers.as_ref(), &staging_random_values_buffer)
    })?;

    let mem_ptr = unsafe {
      device.map_memory(alloc.memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())? as *mut u8
    };

    let mut i = 0;
    let storage_output: [MappedHostBuffer<ComputeOutput>; FRAMES_IN_FLIGHT] =
      storage_output_buffers.map(|buffer| {
        let result = MappedHostBuffer {
          buffer,
          data_ptr: NonNull::new(unsafe {
            mem_ptr.byte_add(alloc.storage_output_offsets[i] as usize)
          } as *mut ComputeOutput)
          .unwrap(),
        };
        i += 1;
        result
      });

    let staging_random_values = MappedHostBuffer {
      buffer: staging_random_values_buffer,
      data_ptr: NonNull::new(unsafe {
        mem_ptr.byte_add(alloc.staging_random_values_offset as usize)
      } as *mut f32)
      .unwrap(),
    };

    Ok(Self {
      memory: alloc.memory,
      storage_output,
      staging_random_values,
    })
  }

  fn allocate_memory(
    device: &Device,
    physical_device: &PhysicalDevice,
    storage_output: [vk::Buffer; FRAMES_IN_FLIGHT],
    staging_new_random_values: vk::Buffer,
  ) -> Result<StagingMemoryAllocation, AllocationError> {
    const TOTAL_BUFFERS: usize = 1 + FRAMES_IN_FLIGHT;
    let storage_output_requirements =
      storage_output.map(|buffer| unsafe { device.get_buffer_memory_requirements(buffer) });
    let staging_new_random_values_requirements =
      unsafe { device.get_buffer_memory_requirements(staging_new_random_values) };

    log::debug!("Allocating compute host memory");
    let allocation = allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE,
      &utility::concatenate_arrays::<TOTAL_BUFFERS, vk::Buffer>(&[
        &storage_output,
        &[staging_new_random_values],
      ]),
      &utility::concatenate_arrays::<TOTAL_BUFFERS, vk::MemoryRequirements>(&[
        &storage_output_requirements,
        &[staging_new_random_values_requirements],
      ]),
      &[],
      &[],
      Self::HOST_MEMORY_PRIORITY,
    )?;

    let mut offsets_iter = allocation.offsets.buffer_offsets().iter();
    let storage_output_offsets =
      utility::fill_array_with_expression!(*offsets_iter.next().unwrap(), FRAMES_IN_FLIGHT);
    let staging_random_values_offset = *offsets_iter.next().unwrap();

    Ok(StagingMemoryAllocation {
      memory: allocation.memory,
      memory_type: allocation.type_index,
      storage_output_offsets,
      staging_random_values_offset,
    })
  }
}
