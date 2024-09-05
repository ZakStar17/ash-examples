use std::{
  marker::PhantomData,
  ops::{BitOr, Range},
  ptr::{self, NonNull},
};

use ash::vk;
use rand::{rngs::ThreadRng, Rng};

use crate::{
  render::{
    allocator::allocate_and_bind_memory,
    create_objs::create_buffer,
    data::MemoryAndType,
    device_destroyable::{
      destroy, fill_destroyable_array_with_expression, DeviceManuallyDestroyed,
    },
    errors::{AllocationError, OutOfMemoryError},
    initialization::device::{Device, PhysicalDevice},
    FRAMES_IN_FLIGHT,
  },
  utility::{self, OnErr},
};

use super::{super::MappedHostBuffer, ComputeHostIO, STAGING_RANDOM_VALUES_COUNT};

#[derive(Debug)]
pub struct HostComputeData {
  // compute_host_io_memory and staging_random_values_memory can be the same

  // storage buffer containing results to be read by the cpu each frame
  pub compute_host_io: [MappedHostBuffer<ComputeHostIO>; FRAMES_IN_FLIGHT],
  // preferably device local
  compute_host_io_memory: MemoryAndType,
  compute_host_io_offsets: [u64; FRAMES_IN_FLIGHT],

  // random values before being copied to device memory
  pub staging_random_values:
    [MappedHostBuffer<[f32; STAGING_RANDOM_VALUES_COUNT]>; FRAMES_IN_FLIGHT],
  // just host visible
  staging_random_values_memory: MemoryAndType,
  staging_random_values_offsets: [u64; FRAMES_IN_FLIGHT],
  staging_random_values_copy_buffer: Box<[f32; STAGING_RANDOM_VALUES_COUNT]>,

  rng: ThreadRng,
}

impl HostComputeData {
  const RANDOM_VALUES_MEMORY_PRIORITY: f32 = 0.3;
  const COMPUTE_HOST_IO_PRIORITY: f32 = 0.8;
}

#[derive(Debug)]
struct MemoryAllocation {
  pub compute_host_io_memory: MemoryAndType,
  pub compute_host_io_offsets: [u64; FRAMES_IN_FLIGHT],

  pub staging_random_values_memory: MemoryAndType,
  pub staging_random_values_offsets: [u64; FRAMES_IN_FLIGHT],
}

impl HostComputeData {
  pub fn create_and_allocate(
    device: &Device,
    physical_device: &PhysicalDevice,
  ) -> Result<Self, AllocationError> {
    let host_io_buffers = fill_destroyable_array_with_expression!(
      device,
      create_buffer(
        device,
        size_of::<ComputeHostIO>() as u64,
        vk::BufferUsageFlags::STORAGE_BUFFER
      ),
      FRAMES_IN_FLIGHT
    )?;

    let random_values_buffers = fill_destroyable_array_with_expression!(
      device,
      create_buffer(
        device,
        (size_of::<f32>() * STAGING_RANDOM_VALUES_COUNT) as u64,
        vk::BufferUsageFlags::TRANSFER_SRC,
      ),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { host_io_buffers.destroy_self(device) })?;

    let alloc = Self::allocate_memory(
      device,
      physical_device,
      host_io_buffers,
      random_values_buffers,
    )
    .on_err(|_| unsafe {
      destroy!(device => host_io_buffers.as_ref(), random_values_buffers.as_ref())
    })?;

    if !physical_device.mem_properties.memory_types[alloc.compute_host_io_memory.type_i as usize]
      .property_flags
      .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
    {
      for offset in alloc.compute_host_io_offsets {
        if offset % physical_device.properties.non_coherent_atom_size != 0 {
          log::warn!("compute_host_io allocation: location offset in memory ({}) is not aligned to non_coherent_atom_size ({})", offset, physical_device.properties.non_coherent_atom_size);
        }
      }
    }

    // offsets account for when memories are equal
    let host_io_mem_ptr = unsafe {
      device.map_memory(
        *alloc.compute_host_io_memory,
        0,
        vk::WHOLE_SIZE,
        vk::MemoryMapFlags::empty(),
      )? as *mut u8
    };
    let random_values_mem_ptr =
      if alloc.compute_host_io_memory != alloc.staging_random_values_memory {
        unsafe {
          device.map_memory(
            *alloc.staging_random_values_memory,
            0,
            vk::WHOLE_SIZE,
            vk::MemoryMapFlags::empty(),
          )? as *mut u8
        }
      } else {
        host_io_mem_ptr
      };

    let mut i = 0;
    let host_io: [MappedHostBuffer<ComputeHostIO>; FRAMES_IN_FLIGHT] =
      host_io_buffers.map(|buffer| {
        let result = MappedHostBuffer {
          buffer,
          data_ptr: NonNull::new(unsafe {
            host_io_mem_ptr.byte_add(alloc.compute_host_io_offsets[i] as usize)
          } as *mut ComputeHostIO)
          .unwrap(),
        };
        i += 1;
        result
      });

    let mut i = 0;
    let random_values: [MappedHostBuffer<[f32; STAGING_RANDOM_VALUES_COUNT]>; FRAMES_IN_FLIGHT] =
      random_values_buffers.map(|buffer| {
        let result = MappedHostBuffer {
          buffer,
          data_ptr: NonNull::new(unsafe {
            random_values_mem_ptr.byte_add(alloc.staging_random_values_offsets[i] as usize)
          } as *mut [f32; STAGING_RANDOM_VALUES_COUNT])
          .unwrap(),
        };
        i += 1;
        result
      });

    // rust considers uninitialized floats UB, but zeroed floats are fine
    // so `let a: f32 = unsafe {MaybeUninit::uninit().assume_init()}` is UB
    // but `let a: f32 = unsafe {MaybeUninit::zeroed().assume_init()}` is not
    // having Box<[MaybeUninit<f32>; STAGING_RANDOM_VALUES_COUNT]> would not require this but
    //    working with uninit values is kinda annoying
    // rand's Fill trait is implemented for [f32] but not [MaybeUninit<f32>]
    let random_values_copy_buffer: Box<[f32; STAGING_RANDOM_VALUES_COUNT]> =
      unsafe { Box::new_zeroed().assume_init() };

    Ok(Self {
      compute_host_io: host_io,
      compute_host_io_memory: alloc.compute_host_io_memory,
      compute_host_io_offsets: alloc.compute_host_io_offsets,
      staging_random_values: random_values,
      staging_random_values_memory: alloc.staging_random_values_memory,
      staging_random_values_offsets: alloc.staging_random_values_offsets,
      staging_random_values_copy_buffer: random_values_copy_buffer,
      rng: rand::thread_rng(),
    })
  }

  // interacting with gpu memory is expensive: function should only be called once data changes
  pub unsafe fn read_compute_io(
    &self,
    frame_i: usize,
    device: &Device,
    physical_device: &PhysicalDevice,
  ) -> Result<ComputeHostIO, OutOfMemoryError> {
    if !physical_device.mem_properties.memory_types[self.compute_host_io_memory.type_i as usize]
      .property_flags
      .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
    {
      let coherent_align = physical_device.properties.non_coherent_atom_size;
      let range = vk::MappedMemoryRange {
        s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
        p_next: ptr::null(),
        memory: *self.compute_host_io_memory,
        offset: utility::round_down_to_power_of_2_u64(
          self.compute_host_io_offsets[frame_i],
          coherent_align,
        ),
        size: utility::round_up_to_power_of_2_u64(
          size_of::<ComputeHostIO>() as u64,
          coherent_align,
        ),
        _marker: PhantomData,
      };
      device.invalidate_mapped_memory_ranges(&[range])?;
    }

    let data = self.compute_host_io[frame_i].data_ptr.read();
    Ok(data)
  }

  pub unsafe fn refresh_random_staging(&mut self, frame_i: usize, range: Range<usize>) {
    let range_size = range.end - range.start; // range is non inclusive

    let mut dst_ptr = self.staging_random_values[frame_i].data_ptr.as_ptr() as *mut f32;
    dst_ptr = dst_ptr.add(range.start);

    let slice = &mut self.staging_random_values_copy_buffer[range];
    // todo: generate random values in a separate thread?
    self.rng.fill(slice);
    ptr::copy_nonoverlapping(slice.as_ptr(), dst_ptr, range_size);
  }

  fn allocate_memory(
    device: &Device,
    physical_device: &PhysicalDevice,
    host_io: [vk::Buffer; FRAMES_IN_FLIGHT],
    random_values: [vk::Buffer; FRAMES_IN_FLIGHT],
  ) -> Result<MemoryAllocation, AllocationError> {
    const TOTAL_BUFFERS: usize = FRAMES_IN_FLIGHT * 2;
    let host_io_requirements =
      host_io.map(|buffer| unsafe { device.get_buffer_memory_requirements(buffer) });
    let random_values_requirements =
      random_values.map(|buffer| unsafe { device.get_buffer_memory_requirements(buffer) });

    log::debug!("Allocating compute host memory");

    let host_io_allocation = allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE
        .bitor(vk::MemoryPropertyFlags::DEVICE_LOCAL)
        .bitor(vk::MemoryPropertyFlags::HOST_COHERENT),
      &host_io,
      &host_io_requirements,
      &[],
      &[],
      Self::COMPUTE_HOST_IO_PRIORITY,
    )
    .map_err(|_err| {
      // try without HOST_COHERENT
      allocate_and_bind_memory(
        device,
        physical_device,
        vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::DEVICE_LOCAL),
        &host_io,
        &host_io_requirements,
        &[],
        &[],
        Self::COMPUTE_HOST_IO_PRIORITY,
      )
    })
    .map_err(|_err| {
      // try now without DEVICE_LOCAL
      allocate_and_bind_memory(
        device,
        physical_device,
        vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_VISIBLE),
        &host_io,
        &host_io_requirements,
        &[],
        &[],
        Self::COMPUTE_HOST_IO_PRIORITY,
      )
    });

    let alloc = match host_io_allocation {
      Ok(host_io_alloc) => {
        let random_values_alloc = allocate_and_bind_memory(
          device,
          physical_device,
          vk::MemoryPropertyFlags::HOST_VISIBLE,
          &random_values,
          &random_values_requirements,
          &[],
          &[],
          Self::RANDOM_VALUES_MEMORY_PRIORITY,
        )?;

        let mut host_io_iter = host_io_alloc.offsets.buffer_offsets().iter();
        let mut random_values_iter = random_values_alloc.offsets.buffer_offsets().iter();
        let host_io_offsets =
          utility::fill_array_with_expression!(*host_io_iter.next().unwrap(), FRAMES_IN_FLIGHT);
        let random_values_offsets = utility::fill_array_with_expression!(
          *random_values_iter.next().unwrap(),
          FRAMES_IN_FLIGHT
        );

        MemoryAllocation {
          compute_host_io_memory: host_io_alloc.into(),
          compute_host_io_offsets: host_io_offsets,
          staging_random_values_memory: random_values_alloc.into(),
          staging_random_values_offsets: random_values_offsets,
        }
      }
      Err(_err) => {
        let general_alloc = allocate_and_bind_memory(
          device,
          physical_device,
          vk::MemoryPropertyFlags::HOST_VISIBLE,
          &utility::concatenate_arrays::<TOTAL_BUFFERS, vk::Buffer>(&[&host_io, &random_values]),
          &utility::concatenate_arrays::<TOTAL_BUFFERS, vk::MemoryRequirements>(&[
            &host_io_requirements,
            &random_values_requirements,
          ]),
          &[],
          &[],
          Self::COMPUTE_HOST_IO_PRIORITY,
        )?;

        let mut offsets_iter = general_alloc.offsets.buffer_offsets().iter();
        let host_io_offsets =
          utility::fill_array_with_expression!(*offsets_iter.next().unwrap(), FRAMES_IN_FLIGHT);
        let random_values_offsets =
          utility::fill_array_with_expression!(*offsets_iter.next().unwrap(), FRAMES_IN_FLIGHT);

        let general_memory = general_alloc.into();

        MemoryAllocation {
          compute_host_io_memory: general_memory,
          compute_host_io_offsets: host_io_offsets,
          staging_random_values_memory: general_memory,
          staging_random_values_offsets: random_values_offsets,
        }
      }
    };

    Ok(alloc)
  }
}

impl DeviceManuallyDestroyed for HostComputeData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.staging_random_values.destroy_self(device);
    self.compute_host_io.destroy_self(device);

    if self.compute_host_io_memory == self.staging_random_values_memory {
      self.compute_host_io_memory.destroy_self(device);
    } else {
      self.compute_host_io_memory.destroy_self(device);
      self.staging_random_values_memory.destroy_self(device);
    }
  }
}
