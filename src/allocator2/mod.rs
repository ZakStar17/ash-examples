use core::fmt;
use std::fmt::Write;

use ash::vk;
use mem_type_assignment::{assign_memory_type_indexes_to_objects_for_allocation, UnassignedToMemoryObjectsData};

use crate::device::{Device, PhysicalDevice};

mod debug_logging;
mod mem_type_assignment;
mod memory_bound;

pub use memory_bound::MemoryBound;
pub use debug_logging::{debug_print_device_memory_info, debug_print_possible_memory_type_assignment};

#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryWithType {
  pub memory: vk::DeviceMemory,
  pub type_index: usize,
}
pub struct AllocationSuccess<const S: usize> {
  memories: [MemoryWithType; vk::MAX_MEMORY_TYPES],
  memory_count: usize,
  // memory index, offset
  obj_to_memory_assignment: [(usize, u64); S],
}

pub enum AllocationError {}

pub fn allocate_memory_by_requirements<const P: usize, const S: usize>(
  device: &Device,
  physical_device: &PhysicalDevice,
  mem_props: [vk::MemoryPropertyFlags; P],
  objs: [&dyn MemoryBound; S],
  obj_labels: Option<[&'static str; S]>,
  priority: f32, // only set if VK_EXT_memory_priority is enabled
) -> Result<AllocationSuccess<S>, AllocationError> {
  let mem_types = physical_device.memory_types();
  let obj_reqs = unsafe { objs.map(|obj| obj.get_memory_requirements(device)) };

  let result =
    assign_memory_type_indexes_to_objects_for_allocation(UnassignedToMemoryObjectsData {
      mem_types,
      mem_props,
      obj_reqs,
      obj_labels,
    });

  match &result {
    Ok((a, b)) => {
      let labels = match &obj_labels {
        Some(labels_arr) => Some(labels_arr.as_slice()),
        None => None,
      };

      let mut table = String::new();
      debug_logging::write_properties_table(
        &mut table,
        mem_types,
        &mem_props,
        &obj_reqs,
        Some(a.as_slice()),
        labels,
      )
      .unwrap();
      log::debug!("{}", table);
    }
    Err(err) => {
      log::debug!("{}", err);
    }
  }

  // let mut working_memory_types = [usize::MAX; vk::MAX_MEMORY_TYPES];
  // let mut working_memory_types_size = 0;
  // for type_i in assigned_memory_types {
  //   if !working_memory_types[0..working_memory_types_size].contains(&type_i) {
  //     working_memory_types[working_memory_types_size] = type_i;
  //     working_memory_types_size += 1;
  //   }
  // }
  // debug_assert_eq!(working_memory_types_size, unique_type_ixs_count);

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
