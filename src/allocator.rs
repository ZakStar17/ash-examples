use core::fmt;
use std::ptr;

use ash::vk;

use crate::device::PhysicalDevice;

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

pub enum AllocationError {
  // no memory type supports all buffers and images
  NoMemoryTypeSupportsAll,
  // allocation size exceeds allowed by device
  TotalSizeExceedsAllowed(u64),
  // allocation size is bigger than each supported heap size
  TooBigForAllSupportedHeaps(u64),
  // not enough memory in all supported heaps
  NotEnoughMemory(u64),
  // generally shouldn't happen
  AllocationVkError(vk::Result),
  BindingVkError(vk::Result),
}

impl fmt::Debug for AllocationError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::NoMemoryTypeSupportsAll => f.write_str(
        "No device memory type supports a combination of all buffers and images memory properties",
      ),
      Self::TotalSizeExceedsAllowed(size) => f.write_fmt(format_args!(
        "Total allocation size ({}) is bigger than allowed by the device",
        size
      )),
      Self::TooBigForAllSupportedHeaps(size) => f.write_fmt(format_args!(
        "Total allocation size ({}) is bigger than each supported heap capacity",
        size
      )),
      Self::NotEnoughMemory(size) => f.write_fmt(format_args!(
        "No memory in all supported heaps for an allocation of this size {}",
        size
      )),
      Self::AllocationVkError(err) => f.write_fmt(format_args!(
        "An error occurred while allocating: {:?}",
        err
      )),
      Self::BindingVkError(err) => f.write_fmt(format_args!(
        "An error occurred while binding memory to objects: {:?}",
        err
      )),
    }
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
  let mut out_of_memory_err = false;
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
          unsafe { device.bind_buffer_memory(buffer, memory, offset) }.map_err(|vk_err| {
            unsafe {
              device.free_memory(memory, None);
            }
            AllocationError::BindingVkError(vk_err)
          })?;
        }
        for (&image, &offset) in images.iter().zip(offsets.image_offsets().iter()) {
          unsafe { device.bind_image_memory(image, memory, offset) }.map_err(|vk_err| {
            unsafe {
              device.free_memory(memory, None);
            }
            AllocationError::BindingVkError(vk_err)
          })?;
        }

        return Ok(PackedAllocation {
          memory,
          memory_size: total_size,
          memory_type: mem_type_i as u32,
          offsets,
        });
      }
      Err(err) => {
        if err == vk::Result::ERROR_OUT_OF_DEVICE_MEMORY {
          out_of_memory_err = true;
          continue;
        }
        return Err(AllocationError::AllocationVkError(err));
      }
    }
  }

  if out_of_memory_err {
    return Err(AllocationError::NotEnoughMemory(total_size));
  }
  if heap_capacity_exceeded {
    return Err(AllocationError::TooBigForAllSupportedHeaps(total_size));
  }
  Err(AllocationError::NoMemoryTypeSupportsAll)
}
