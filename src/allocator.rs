use std::ptr;

use ash::vk;

use crate::{
  device::PhysicalDevice,
  errors::{AllocationError, OutOfMemoryError},
};

pub struct PackedAllocation {
  pub memory: vk::DeviceMemory,
  pub memory_size: u64,
  pub memory_type: u32,
  pub offsets: AllocationOffsets,
}

pub struct AllocationOffsets {
  buffers_len: usize,
  offsets: Box<[u64]>,
}

impl AllocationOffsets {
  pub fn buffer_offsets(&self) -> &[u64] {
    &self.offsets[0..self.buffers_len]
  }

  pub fn image_offsets(&self) -> &[u64] {
    &self.offsets[self.buffers_len..]
  }
}

// allocates vk::DeviceMemory and binds buffers and images to it with correct alignments
pub fn allocate_and_bind_memory(
  device: &ash::Device,
  physical_device: &PhysicalDevice,
  memory_properties: vk::MemoryPropertyFlags,
  buffers: &[vk::Buffer],
  buffers_memory_requirements: &[vk::MemoryRequirements],
  images: &[vk::Image],
  images_memory_requirements: &[vk::MemoryRequirements],
) -> Result<PackedAllocation, AllocationError> {
  let mut mem_types_bitmask = u32::MAX;
  let mut total_size = 0;
  let offsets: Box<[u64]> = buffers_memory_requirements
    .iter()
    .chain(images_memory_requirements.iter())
    .map(|mem_requirements| {
      mem_types_bitmask &= mem_requirements.memory_type_bits;

      // align internal offset to follow memory requirements
      let mut offset = total_size;
      let align_error = offset % mem_requirements.alignment;
      if align_error > 0 {
        offset += mem_requirements.alignment - align_error;
      }

      total_size = offset + mem_requirements.size;
      offset
    })
    .collect();

  let offsets = AllocationOffsets {
    offsets,
    buffers_len: buffers.len(),
  };

  if total_size >= physical_device.properties.p11.max_memory_allocation_size {
    return Err(AllocationError::TotalSizeExceedsAllowed(total_size));
  }

  let mut heap_capacity_exceeded = false;
  let mut out_of_host_memory = false;
  let mut out_of_device_memory = false;
  for (mem_type_i, _mem_type) in
    physical_device.iterate_memory_types_with_unique_heaps(mem_types_bitmask, memory_properties)
  {
    let heap_size = physical_device.memory_type_heap(mem_type_i).size;
    if total_size >= heap_size {
      heap_capacity_exceeded = true;
      continue;
    }

    let allocate_info = vk::MemoryAllocateInfo {
      s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
      p_next: ptr::null(),
      allocation_size: total_size,
      memory_type_index: mem_type_i as u32,
    };
    match unsafe { device.allocate_memory(&allocate_info, None) } {
      Ok(memory) => {
        for (&buffer, &offset) in buffers.iter().zip(offsets.buffer_offsets().iter()) {
          if let Err(vk_err) = unsafe { device.bind_buffer_memory(buffer, memory, offset) } {
            unsafe {
              device.free_memory(memory, None);
            }
            return Err(vk_err.into());
          }
        }
        for (&image, &offset) in images.iter().zip(offsets.image_offsets().iter()) {
          if let Err(vk_err) = unsafe { device.bind_image_memory(image, memory, offset) } {
            unsafe {
              device.free_memory(memory, None);
            }
            return Err(vk_err.into());
          }
        }

        return Ok(PackedAllocation {
          memory,
          memory_size: total_size,
          memory_type: mem_type_i as u32,
          offsets,
        });
      }
      Err(err) => {
        if err == vk::Result::ERROR_OUT_OF_HOST_MEMORY {
          out_of_host_memory = true;
          continue;
        }
        if err == vk::Result::ERROR_OUT_OF_DEVICE_MEMORY {
          out_of_device_memory = true;
          continue;
        }
        panic!();
      }
    }
  }

  if out_of_host_memory {
    return Err(AllocationError::NotEnoughMemory(
      OutOfMemoryError::OutOfHostMemory,
    ));
  }
  if out_of_device_memory {
    return Err(AllocationError::NotEnoughMemory(
      OutOfMemoryError::OutOfDeviceMemory,
    ));
  }
  if heap_capacity_exceeded {
    return Err(AllocationError::TooBigForAllSupportedHeaps(total_size));
  }

  // allocation for loop never ran
  Err(AllocationError::NoMemoryTypeSupportsAll)
}
