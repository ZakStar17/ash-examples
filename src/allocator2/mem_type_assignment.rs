use std::fmt;

use ash::vk;

#[derive(Debug)]
pub struct UnassignedToMemoryObjectsData<'a, const P: usize, const S: usize> {
  pub mem_types: &'a [vk::MemoryType],
  // assign only to mem_types that contain the following sets of memory properties
  // try to assign first to the memory properties set mem_props[0], then mem_props[1] and so on
  pub mem_props: [vk::MemoryPropertyFlags; P],
  // MemoryBound::get_memory_requirements
  pub obj_reqs: [vk::MemoryRequirements; S],
  pub obj_labels: Option<[&'static str; S]>,
}

impl<'a, const P: usize, const S: usize> fmt::Display for UnassignedToMemoryObjectsData<'a, P, S> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let labels = match &self.obj_labels {
      Some(labels_arr) => Some(labels_arr.as_slice()),
      None => None,
    };
    super::debug_logging::write_properties_table(
      f,
      &self.mem_types,
      &self.mem_props,
      &self.obj_reqs,
      None,
      labels,
    )
  }
}

#[derive(Debug)]
pub enum MemoryAssignmentError<'a, const P: usize, const S: usize> {
  AllPropertiesUnsupported,
  ObjectIncompatibleWithAllProperties(UnassignedToMemoryObjectsData<'a, P, S>, usize),
}

impl<'a, const P: usize, const S: usize> fmt::Display for MemoryAssignmentError<'a, P, S> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::AllPropertiesUnsupported => {
        f.write_str("No given memory property set is supported by the system")?;
      }
      Self::ObjectIncompatibleWithAllProperties(data, i) => {
        f.write_str(
          "One or more objects are not compatible with any given sets of memory properties.\n",
        )?;
        f.write_fmt(format_args!("{}", data))?;
        f.write_fmt(format_args!(
          " - o{} {}is incompatible with all given property sets",
          i,
          if let Some(labels) = data.obj_labels {
            format!("\"{}\" ", labels[*i])
          } else {
            "".to_owned()
          }
        ))?;
      }
    }
    Ok(())
  }
}

// assigns an memory type index to each object specified in <requirements> based on supported
// memory types in a way that preferably objects get assigned to the same memory type and in
// the desired memory properties
//
// note: memory types bitmask in vulkan are ordered from right to left, so the first type index
// is the last in memory (so bitmask & 1 > 0 tests if the first type is compatible)
//
// todo: doesn't test if requirements exceed memory heap capacity
pub fn assign_memory_type_indexes_to_objects_for_allocation<const P: usize, const S: usize>(
  data: UnassignedToMemoryObjectsData<P, S>,
) -> Result<([usize; S], usize), MemoryAssignmentError<P, S>> {
  // bitmask of memory types that are supported by the given desired properties
  let bit_switch = 1 << (data.mem_types.len() - 1);
  let supported_properties = data.mem_props.map(|p: vk::MemoryPropertyFlags| {
    let mut support_bitmask: u32 = 0;
    for t in data.mem_types {
      support_bitmask >>= 1;
      if t.property_flags.contains(p) {
        support_bitmask |= bit_switch; // switch last bit to 1
      }
    }
    support_bitmask
  });

  if supported_properties.iter().all(|&bitmask| bitmask == 0) {
    // no desired properties are supported by the system
    return Err(MemoryAssignmentError::AllPropertiesUnsupported);
  }

  for (i, req) in data.obj_reqs.iter().enumerate() {
    if !supported_properties
      .iter()
      .any(|&p| p & req.memory_type_bits > 0)
    {
      // some object is unsupported by all given memory properties
      return Err(MemoryAssignmentError::ObjectIncompatibleWithAllProperties(
        data, i,
      ));
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
        let mask = supported_type_ixs & data.obj_reqs[i].memory_type_bits;
        if mask > 0 {
          working_obj_ixs_and_masks[working_obj_size] = (i, mask);
          working_obj_size += 1;
        }
      }
    }
    if working_obj_size == 0 {
      continue;
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
      while all_type_bitmask & 1 == 0 {
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

  assert!(remaining == 0);

  Ok((assigned, unique_type_count))
}
