use core::fmt;
use std::ptr;

use ash::vk;

use super::initialization::PhysicalDevice;


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

pub enum PackedAllocationError {
  MemoryTypeNotSupported,
  TotalSizeExceedsAllowed(u64),
  TotalSizeExceedsHeapSize(u64),
  VkError(vk::Result),
}

impl fmt::Debug for PackedAllocationError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
        Self::MemoryTypeNotSupported => f.write_str(
          "No device memory type supports all requested buffers and images with the requested properties"
        ),
        Self::TotalSizeExceedsAllowed(size) => f.write_fmt(
          format_args!("Total allocation size ({}) is bigger than the value allowed by the device", size)
        ),
        Self::TotalSizeExceedsHeapSize(size) => f.write_fmt(
          format_args!("A allocation memory type was found but the total allocation size ({}) exceeds its heap capacity", size)
        ),
        Self::VkError(err) => err.fmt(f)
      }
  }
}

// allocates vk::DeviceMemory and binds buffers and images to it with correct alignments
pub fn allocate_and_bind_memory(
  device: &ash::Device,
  physical_device: &PhysicalDevice,
  required_memory_properties: vk::MemoryPropertyFlags,
  optional_memory_properties: vk::MemoryPropertyFlags,
  buffers: &[vk::Buffer],
  images: &[vk::Image],
) -> Result<PackedAllocation, PackedAllocationError> {
  let mut req_mem_type_bits = 0;
  let mut total_size = 0;
  let offsets: Box<[u64]> = buffers
    .iter()
    .map(|&buffer| unsafe { device.get_buffer_memory_requirements(buffer) })
    .chain(
      images
        .iter()
        .map(|&image| unsafe { device.get_image_memory_requirements(image) }),
    )
    .map(|mem_requirements| {
      req_mem_type_bits |= mem_requirements.memory_type_bits;

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

  // in this case it can be possible to sub allocate
  if total_size >= physical_device.get_max_memory_allocation_size() {
    return Err(PackedAllocationError::TotalSizeExceedsAllowed(total_size));
  }

  let memory_type = physical_device
    .find_optimal_memory_type(
      req_mem_type_bits,
      required_memory_properties,
      optional_memory_properties,
    )
    .or(Err(PackedAllocationError::MemoryTypeNotSupported))?;

  let heap_size = physical_device.get_memory_type_heap(memory_type).size;
  if total_size >= heap_size {
    return Err(PackedAllocationError::TotalSizeExceedsHeapSize(total_size));
  }

  let allocate_info = vk::MemoryAllocateInfo {
    s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
    p_next: ptr::null(),
    allocation_size: total_size,
    memory_type_index: memory_type,
  };
  let memory = unsafe { device.allocate_memory(&allocate_info, None) }
    .map_err(|vk_err| PackedAllocationError::VkError(vk_err))?;

  for (&buffer, &offset) in buffers.iter().zip(offsets.buffer_offsets().iter()) {
    unsafe { device.bind_buffer_memory(buffer, memory, offset) }
      .map_err(|vk_err| PackedAllocationError::VkError(vk_err))?;
  }
  for (&image, &offset) in images.iter().zip(offsets.image_offsets().iter()) {
    unsafe { device.bind_image_memory(image, memory, offset) }
      .map_err(|vk_err| PackedAllocationError::VkError(vk_err))?;
  }

  Ok(PackedAllocation {
    memory,
    memory_size: total_size,
    memory_type,
    offsets,
  })
}
