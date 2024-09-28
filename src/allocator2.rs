use core::fmt;
use std::fmt::Write;

use ash::vk;

use crate::{
  device::{Device, PhysicalDevice},
  errors::{AllocationError, OutOfMemoryError},
};

#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryWithType {
  pub memory: vk::DeviceMemory,
  pub type_index: usize,
}
pub struct AllocationSuccessResult<const S: usize> {
  memories: [MemoryWithType; vk::MAX_MEMORY_TYPES],
  memory_count: usize,
  // memory index, offset
  obj_to_memory_assignment: [(usize, u64); S],
}

pub trait MemoryBound {
  unsafe fn bind(
    &self,
    device: &ash::Device,
    memory: vk::DeviceMemory,
    offset: u64,
  ) -> Result<(), OutOfMemoryError>;
  unsafe fn get_memory_requirements(&self, device: &ash::Device) -> vk::MemoryRequirements;
}

impl MemoryBound for vk::Buffer {
  unsafe fn bind(
    &self,
    device: &ash::Device,
    memory: vk::DeviceMemory,
    offset: u64,
  ) -> Result<(), OutOfMemoryError> {
    device
      .bind_buffer_memory(*self, memory, offset)
      .map_err(|err| err.into())
  }

  unsafe fn get_memory_requirements(&self, device: &ash::Device) -> vk::MemoryRequirements {
    device.get_buffer_memory_requirements(*self)
  }
}

impl MemoryBound for vk::Image {
  unsafe fn bind(
    &self,
    device: &ash::Device,
    memory: vk::DeviceMemory,
    offset: u64,
  ) -> Result<(), OutOfMemoryError> {
    device
      .bind_image_memory(*self, memory, offset)
      .map_err(|err| err.into())
  }

  unsafe fn get_memory_requirements(&self, device: &ash::Device) -> vk::MemoryRequirements {
    device.get_image_memory_requirements(*self)
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
pub fn assign_memory_type_indexes_to_objects_for_allocation<const P: usize, const S: usize>(
  system_memory_types: &[vk::MemoryType; vk::MAX_MEMORY_TYPES],
  requirements: [vk::MemoryRequirements; S],
  // assign only to types that contain the following properties
  // try to assign first to memory with desired_properties[0], and only try subsequent assignments
  // if that memory is not supported by the system or not supported by the object requirements
  // the function returns an error if none of the properties can be supported for some object
  desired_properties: [vk::MemoryPropertyFlags; P],
) -> Result<([usize; S], usize), ()> {
  // bitmask of memory types that are supported by the given desired properties
  let supported_properties = desired_properties.map(|p: vk::MemoryPropertyFlags| {
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
  let mut unique_type_count = 0;

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

      unique_type_count += 1;
    } else {
      let mut type_i_counter = [0usize; 32];
      for &(_, mask) in cur_working_objs_ixs_and_masks {
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
        debug_assert!(type_i_counter[cur_max_i] > 0);
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

        unique_type_count += 1;
      }
    }
  }

  if remaining > 0 {
    panic!();
  }

  Ok((assigned, unique_type_count))
}

pub fn debug_print_device_memory_info(
  mem_properties: &vk::PhysicalDeviceMemoryProperties,
) -> fmt::Result {
  let mut output = String::new();

  output.write_fmt(format_args!(
    "\nAvailable memory heaps: ({} heaps, {} memory types)\n",
    mem_properties.memory_heap_count, mem_properties.memory_type_count
  ))?;
  for heap_i in 0..mem_properties.memory_heap_count {
    let heap = mem_properties.memory_heaps[heap_i as usize];
    let heap_flags = if heap.flags.is_empty() {
      String::from("no heap flags")
    } else {
      format!("heap flags [{:?}]", heap.flags)
    };

    output.write_fmt(format_args!(
      "    {} -> {}MiB with {} and attributed memory types:\n",
      heap_i,
      heap.size / 1000000,
      heap_flags
    ))?;
    for type_i in 0..mem_properties.memory_type_count {
      let mem_type = mem_properties.memory_types[type_i as usize];
      if mem_type.heap_index != heap_i {
        continue;
      }

      let flags = mem_type.property_flags;
      output.write_fmt(format_args!(
        "        {} -> {}\n",
        type_i,
        if flags.is_empty() {
          "<no flags>".to_owned()
        } else {
          format!("[{:?}]", flags)
        }
      ))?;
    }
  }
  log::debug!("{}", output);

  Ok(())
}

fn digit_count(mut n: usize) -> usize {
  if n == 0 {
    return 1;
  }
  let mut count = 0;
  while n != 0 {
    n /= 10;
    count += 1;
  }
  return count;
}

pub fn debug_print_obj_memory_requirements<const P: usize, const S: usize>(
  device: &Device,
  physical_device: &PhysicalDevice,
  desired_memory_properties: [vk::MemoryPropertyFlags; P],
  objs: [&dyn MemoryBound; S],
  obj_labels: Option<[&'static str; S]>,
) -> fmt::Result {
  assert!(desired_memory_properties.len() > 0);
  let mut output = format!(
    "\nAttempting to allocate memory for <{}> objects in memory types with {}.\n",
    S,
    if desired_memory_properties.len() == 1 {
      format!(
        "the following properties: \"{:?}\"",
        desired_memory_properties[0]
      )
    } else {
      format!(
        "one of the following sets of properties: {:?}, in increased preference",
        desired_memory_properties.map(|prop| if prop.is_empty() {"<no properties>".to_owned()} else {format!("{:?}", prop)})
      )
    }
  );

  let memory_types = physical_device.memory_types();
  let mem_requirements = unsafe { objs.map(|obj| obj.get_memory_requirements(device)) };

  output.write_fmt(format_args!(
    "Device memory contains {} distinct memory types.
Label:
    <mi>: memory type <i>
    <px>: property <i>
    <oi>: object <i>
    \"#\": supported
    \".\": not supported\n",
    memory_types.len()
  ))?;

  let col_width = 2.max(digit_count(P)).max(digit_count(S));

  let mut line = String::new();
  for i in 0..desired_memory_properties.len() {
    line.write_fmt(format_args!("p{:<1$} ", i, col_width))?;
  }
  line.write_str("        ")?;
  for i in 0..mem_requirements.len() {
    line.write_fmt(format_args!("o{:<1$} ", i, col_width))?;
  }
  line.write_char('\n')?;
  output.write_str(&line)?;
  line.clear();

  for i in 0..memory_types.len() {
    for prop in desired_memory_properties {
      line.write_fmt(format_args!(
        "{:<1$} ",
        if memory_types[i].property_flags.contains(prop) {
          '#'
        } else {
          '.'
        },
        col_width + 1
      ))?;
    }
    line.write_fmt(format_args!("| m{:<3}| ", i))?;
    for req in mem_requirements {
      line.write_fmt(format_args!(
        "{:<1$} ",
        if req.memory_type_bits & i as u32 > 0 {
          '#'
        } else {
          '.'
        },
        col_width + 1
      ))?;
    }

    line.write_char('\n')?;
    output.write_str(&line)?;
    line.clear();
  }

  if let Some(obj_labels) = obj_labels {
    for (i, label) in obj_labels.iter().enumerate() {
      output.write_fmt(format_args!("o{}: {:?}\n", i, label))?;
    }
  }

  log::debug!("{}", output);
  Ok(())
}

pub fn allocate_memory_by_requirements<const P: usize, const S: usize>(
  device: &Device,
  physical_device: &PhysicalDevice,
  obj_memory_requirements: [vk::MemoryRequirements; S],
  desired_memory_properties: [vk::MemoryPropertyFlags; P],
  priority: f32, // only set if VK_EXT_memory_priority is enabled
) -> Result<AllocationSuccessResult<S>, AllocationError> {
  let memory_types = physical_device.mem_properties.memory_types;

  let (assigned_memory_types, unique_type_ixs_count) =
    assign_memory_type_indexes_to_objects_for_allocation(
      &memory_types,
      obj_memory_requirements,
      desired_memory_properties,
    )
    .unwrap();

  let mut working_memory_types = [usize::MAX; vk::MAX_MEMORY_TYPES];
  let mut working_memory_types_size = 0;
  for type_i in assigned_memory_types {
    if !working_memory_types[0..working_memory_types_size].contains(&type_i) {
      working_memory_types[working_memory_types_size] = type_i;
      working_memory_types_size += 1;
    }
  }
  debug_assert_eq!(working_memory_types_size, unique_type_ixs_count);

  todo!()
  // let mut allocation_result = [(usize::MAX, u64::MAX); S];
  // let mut memories = [MemoryWithType {
  //   memory: vk::DeviceMemory::null(),
  //   type_index: usize::MAX,
  // }; vk::MAX_MEMORY_TYPES];
  // for (mem_i, &type_i) in working_memory_types[0..working_memory_types_size]
  //   .iter()
  //   .enumerate()
  // {
  //   let mut total_size = 0;
  //   for i in 0..S {
  //     if assigned_memory_types[i] == type_i {
  //       let offset =
  //         utility::round_up_to_power_of_2_u64(total_size, obj_memory_requirements[i].alignment);
  //       total_size = offset + obj_memory_requirements[i].size;
  //       allocation_result[i].1 = offset;
  //     }
  //   }

  //   let mut allocate_info = vk::MemoryAllocateInfo {
  //     s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
  //     p_next: ptr::null(),
  //     allocation_size: total_size,
  //     memory_type_index: type_i as u32,
  //     _marker: PhantomData,
  //   };
  //   if device.enabled_extensions.memory_priority {
  //     let priority_info = vk::MemoryPriorityAllocateInfoEXT::default().priority(priority);
  //     allocate_info.p_next =
  //       &priority_info as *const vk::MemoryPriorityAllocateInfoEXT as *const c_void;
  //   }
  //   let memory = unsafe { device.allocate_memory(&allocate_info, None) }.unwrap();
  //   if let Some(loader) = device.pageable_device_local_memory_loader.as_ref() {
  //     unsafe {
  //       // set manually priority as well for the pageable_device_local_memory extension
  //       // only raw function pointers for now
  //       (loader.fp().set_device_memory_priority_ext)(device.handle(), memory, priority);
  //     }
  //   }
  //   memories[mem_i] = MemoryWithType {
  //     memory,
  //     type_index: type_i,
  //   };

  //   for i in 0..S {
  //     if assigned_memory_types[i] == type_i {
  //       allocation_result[i].0 = mem_i;
  //     }
  //   }
  // }

  // Ok(AllocationSuccessResult {
  //   memories,
  //   memory_count: working_memory_types_size,
  //   obj_to_memory_assignment: allocation_result,
  // })
}
