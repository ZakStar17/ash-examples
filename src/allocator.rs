use std::{ffi::c_void, marker::PhantomData, ptr};

use ash::vk;

use crate::{
  device::{Device, PhysicalDevice},
  errors::{AllocationError, OutOfMemoryError},
  utility,
};

#[allow(dead_code)]
pub struct PackedAllocation {
  pub memory: vk::DeviceMemory,
  pub size: u64,
  pub type_index: u32,
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

// assigns an memory type index to each object specified in <requirements> based on supported
// memory types in a way that preferably objects get assigned to the same memory type and in
// the desired memory properties
//
// note: memory types bitmask in vulkan are ordered from right to left, so the first type index
// is the last in memory (so bitmask & 1 > 0 tests if the first type is compatible)
//
// todo: doesn't actually tests for memory heap compatibility
fn assign_memory_type_indexes_to_objects_for_allocation<const P: usize, const S: usize>(
  system_memory_types: &[vk::MemoryType; vk::MAX_MEMORY_TYPES],
  requirements: [vk::MemoryRequirements; S],
  // assign only to types that contain the following properties
  // try to assign first to memory with desired_properties[0], and only try subsequent assignments
  // if that memory is not supported by the system or not supported by the object requirements
  // the function returns an error if none of the properties can be supported for some object
  desired_properties: [vk::MemoryPropertyFlags; P],
) -> Result<[usize; S], ()> {
  // todo: write proper errors

  // bitmask of memory types that are supported by the given desired properties
  let supported_properties = desired_properties.map(|p| {
    let mut support_bitmask: u32 = 0;
    for t in system_memory_types {
      support_bitmask <<= 1;
      if t.property_flags.contains(p) {
        support_bitmask |= 1; // switch last bit to 1
      }
    }
    support_bitmask
  });

  if supported_properties.iter().all(|&bitmask| bitmask == 0) {
    // no desired properties are supported by the system
    return Err(());
  }

  for req in requirements {
    if !supported_properties
      .iter()
      .any(|&p| p & req.memory_type_bits > 0)
    {
      // some object is unsupported by all given memory properties
      return Err(());
    }
  }

  let mut assigned = [usize::MAX; S];
  let mut remaining = S;

  let mut working_obj_ixs_and_masks = [(0usize, 0u32); S];
  for (_prop_i, supported_type_ixs) in supported_properties.into_iter().enumerate() {
    if remaining == 0 {
      break;
    }

    let mut working_obj_size = 0;
    for (i, obj) in assigned.into_iter().enumerate() {
      if obj == usize::MAX {
        let mask = supported_type_ixs & requirements[i].memory_type_bits;
        if mask > 0 {
          working_obj_ixs_and_masks[working_obj_size] = (i, mask);
          working_obj_size += 1;
        }
      }
    }
    remaining -= working_obj_size;
    let cur_working_objs_ixs_and_masks = &working_obj_ixs_and_masks[0..working_obj_size];

    let mut all_type_bitmask = cur_working_objs_ixs_and_masks
      .iter()
      .fold(u32::MAX, |acc, (_, mask)| acc & mask);
    if all_type_bitmask > 0 {
      // all objects are compatible with some memory type

      // find first bit
      let mut type_i = 0;
      while all_type_bitmask & 1 == 1 {
        all_type_bitmask >>= 1;
        type_i += 1;
      }
      debug_assert!(supported_type_ixs & (1 << type_i) > 0);
      for &(i, mask) in cur_working_objs_ixs_and_masks {
        debug_assert!(mask & (1 << type_i) > 0);
        assigned[i] = type_i;
      }
    } else {
      let mut type_i_counter = [0usize; 32];
      for &(i, mask) in cur_working_objs_ixs_and_masks {
        let mut bit_i = 1;
        for i in 0..32 {
          if mask & bit_i > 0 {
            // bit_ith bit is 1
            type_i_counter[i] += 1;
          }
          bit_i <<= 1;
        }
      }

      // assign indexes to each object in a way to create the minimum amount of used type indexes

      // probably doesn't always work (selecting the most common supported type), so, for now
      // todo
      let mut cur_remaining = working_obj_size;
      while cur_remaining > 0 {
        let cur_max_i = type_i_counter
          .iter()
          .enumerate()
          .max_by(|x, y| x.1.cmp(y.1))
          .unwrap()
          .0;
        for &(obj_i, mask) in cur_working_objs_ixs_and_masks {
          if mask & (1 << cur_max_i) > 0 && assigned[obj_i] == usize::MAX {
            assigned[obj_i] = cur_max_i;
            cur_remaining -= 1;

            let mut bit_i = 1;
            for i in 0..32 {
              if mask & bit_i > 0 {
                // bit_ith bit is 1
                type_i_counter[i] -= 1;
              }
              bit_i <<= 1;
            }
          }
        }
      }
    }
  }

  if remaining > 0 {
    return Err(());
  }

  Ok(assigned)
}

pub fn allocate_memory_by_requirements<const P: usize, const S: usize>(
  device: &Device,
  physical_device: &PhysicalDevice,
  obj_memory_requirements: [vk::MemoryRequirements; S],
  desired_memory_properties: [vk::MemoryPropertyFlags; P],
  priority: f32, // only set if VK_EXT_memory_priority is enabled
) {
  let memory_types = physical_device.mem_properties.memory_types;

  let assigned_memory_types = assign_memory_type_indexes_to_objects_for_allocation(
    &memory_types,
    obj_memory_requirements,
    desired_memory_properties,
  )
  .unwrap();
}

// allocates vk::DeviceMemory and binds buffers and images to it with correct alignments
// memory frequently written by the GPU should require higher priority
pub fn allocate_and_bind_memory(
  device: &Device,
  physical_device: &PhysicalDevice,
  memory_properties: vk::MemoryPropertyFlags,
  buffers: &[vk::Buffer],
  buffers_memory_requirements: &[vk::MemoryRequirements],
  images: &[vk::Image],
  images_memory_requirements: &[vk::MemoryRequirements],
  priority: f32, // only set if VK_EXT_memory_priority is enabled
) -> Result<PackedAllocation, AllocationError> {
  let mut mem_types_bitmask = u32::MAX;
  let mut total_size = 0;
  let offsets: Box<[u64]> = buffers_memory_requirements
    .iter()
    .chain(images_memory_requirements.iter())
    .map(|mem_requirements| {
      mem_types_bitmask &= mem_requirements.memory_type_bits;

      debug_assert!(mem_requirements.alignment % 2 == 0);

      // align internal offset to follow memory requirements
      let mut offset = total_size;
      // strip right-most <alignment> bits from offset (alignment is always a power of 2)
      let align_error = offset & (mem_requirements.alignment - 1);
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

  if total_size >= physical_device.max_memory_allocation_size {
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

    let mut allocate_info = vk::MemoryAllocateInfo {
      s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
      p_next: ptr::null(),
      allocation_size: total_size,
      memory_type_index: mem_type_i as u32,
      _marker: PhantomData,
    };

    if device.enabled_extensions.memory_priority {
      let priority_info = vk::MemoryPriorityAllocateInfoEXT::default().priority(priority);
      allocate_info.p_next =
        &priority_info as *const vk::MemoryPriorityAllocateInfoEXT as *const c_void;
    }
    match unsafe { device.allocate_memory(&allocate_info, None) } {
      Ok(memory) => {
        if let Some(loader) = device.pageable_device_local_memory_loader.as_ref() {
          unsafe {
            // set manually priority as well for the  pageable_device_local_memory extension
            // only raw function pointers for now
            (loader.fp().set_device_memory_priority_ext)(device.handle(), memory, priority);
          }
        }

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
          size: total_size,
          type_index: mem_type_i as u32,
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
