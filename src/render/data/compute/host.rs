use std::ptr::NonNull;

use ash::vk;

use crate::{
  render::{
    allocator::allocate_and_bind_memory,
    create_objs::create_buffer,
    device_destroyable::{
      destroy, fill_destroyable_array_with_expression, DeviceManuallyDestroyed,
    },
    errors::AllocationError,
    initialization::device::{Device, PhysicalDevice},
    FRAMES_IN_FLIGHT,
  },
  utility::{self, OnErr},
};

use super::super::MappedHostBuffer;

// host accessible data after shader dispatch
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ComputeOutput {
  // 1 if colliding with bullet, 0 otherwise
  colliding: u32,
  // number of random values remaining for use in ComputeData::device_random_values
  random_values_left: u32,
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
struct MemoryAllocation {
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
    let storage_output_buffers = fill_destroyable_array_with_expression!(
      device,
      create_buffer(
        device,
        size_of::<ComputeOutput>() as u64,
        vk::BufferUsageFlags::STORAGE_BUFFER
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
  ) -> Result<MemoryAllocation, AllocationError> {
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

    Ok(MemoryAllocation {
      memory: allocation.memory,
      memory_type: allocation.type_index,
      storage_output_offsets,
      staging_random_values_offset,
    })
  }
}

impl DeviceManuallyDestroyed for HostComputeData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.staging_random_values.destroy_self(device);
    self.storage_output.destroy_self(device);

    self.memory.destroy_self(device);
  }
}
