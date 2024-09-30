use std::fmt::{self, Write};

use ash::vk;

use crate::{
  allocator2::mem_type_assignment::{
    assign_memory_type_indexes_to_objects_for_allocation, UnassignedToMemoryObjectsData,
  },
  device::{Device, PhysicalDevice},
};

use super::{mem_type_assignment::MemoryAssignmentError, MemoryBound};

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

pub fn write_properties_table(
  f: &mut dyn fmt::Write,
  mem_types: &[vk::MemoryType],
  mem_props: &[vk::MemoryPropertyFlags],
  obj_reqs: &[vk::MemoryRequirements],
  obj_assigned: Option<&[usize]>,
  obj_labels: Option<&[&'static str]>,
) -> fmt::Result {
  // todo:
  // print mem_props bit names
  // print number of assigned objects to each mem_type

  if let Some(obj_labels) = obj_labels {
    debug_assert_eq!(obj_reqs.len(), obj_labels.len());
  }
  debug_assert!(mem_types.len() > 0);

  if mem_props.len() == 0 && obj_reqs.len() == 0 {
    return Ok(());
  }

  let mem_types_col_width = digit_count(mem_types.len());
  let mem_props_col_width = digit_count(mem_props.len());
  let obj_reqs_col_width = digit_count(obj_reqs.len());

  f.write_fmt(format_args!(
    "Label:
    <mi>: memory type <i>{}{}{}
    \"#\": supported
    \".\": not supported\n",
    if mem_props.len() > 0 {
      "\n    <px>: property <i>"
    } else {
      ""
    },
    if obj_reqs.len() > 0 {
      "\n    <oi>: object <i>"
    } else {
      ""
    },
    if obj_assigned.is_some() {
      "\n    \"A\": assigned"
    } else {
      ""
    }
  ))?;

  // write first line
  let mut line = String::new();
  for i in 0..mem_props.len() {
    line.write_fmt(format_args!("p{:<1$} ", i, mem_props_col_width))?;
  }
  for _ in 0..(2
    + mem_types_col_width
    + if mem_props.len() > 0 { 2 } else { 0 }
    + if obj_reqs.len() > 0 { 2 } else { 0 })
  {
    line.write_char(' ')?;
  }
  for i in 0..obj_reqs.len() {
    line.write_fmt(format_args!("o{:<1$} ", i, obj_reqs_col_width))?;
  }
  let _ = line.pop();
  line.write_char('\n')?;
  f.write_str(&line)?;
  line.clear();

  for mem_type_i in 0..mem_types.len() {
    for &prop in mem_props {
      line.write_fmt(format_args!(
        "{:<1$} ",
        if mem_types[mem_type_i].property_flags.contains(prop) {
          '#'
        } else {
          '.'
        },
        mem_props_col_width + 1
      ))?;
    }
    if mem_props.len() > 0 {
      line.write_str("| ")?;
    }
    line.write_fmt(format_args!("m{:<1$} ", mem_type_i, mem_types_col_width))?;
    if obj_reqs.len() > 0 {
      line.write_str("| ")?;
    }
    if let Some(assigned) = obj_assigned {
      for (&req, &assigned_type_i) in obj_reqs.iter().zip(assigned.iter()) {
        line.write_fmt(format_args!(
          "{:<1$} ",
          if assigned_type_i == mem_type_i {
            assert!(req.memory_type_bits & (1 << mem_type_i as u32) > 0);
            'A'
          } else if req.memory_type_bits & (1 << mem_type_i as u32) > 0 {
            '#'
          } else {
            '.'
          },
          obj_reqs_col_width + 1
        ))?;
      }
    } else {
      for req in obj_reqs {
        line.write_fmt(format_args!(
          "{:<1$} ",
          if req.memory_type_bits & (1 << mem_type_i as u32) > 0 {
            '#'
          } else {
            '.'
          },
          obj_reqs_col_width + 1
        ))?;
      }
    }
    let _ = line.pop();
    line.write_char('\n')?;
    f.write_str(&line)?;
    line.clear();
  }

  if let Some(obj_labels) = obj_labels {
    for (i, label) in obj_labels.iter().enumerate() {
      f.write_fmt(format_args!("o{}: {:?}\n", i, label))?;
    }
  }

  Ok(())
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

pub fn display_mem_assignment_result<const P: usize, const S: usize>(
  f: &mut dyn fmt::Write,
  result: Result<([usize; S], usize), MemoryAssignmentError<P, S>>,
  mem_types: &[vk::MemoryType],
  mem_props: &[vk::MemoryPropertyFlags],
  obj_reqs: &[vk::MemoryRequirements],
  obj_labels: Option<&[&'static str]>,
) -> fmt::Result {
  match result {
    Ok((assigned, unique_type_count)) => {
      f.write_fmt(format_args!(
        "Allocation result: success. Objects got assigned to {} unique memory type{}.\n",
        unique_type_count,
        if unique_type_count == 1 { "" } else { "s" }
      ))?;
      write_properties_table(
        f,
        mem_types,
        &mem_props,
        &obj_reqs,
        Some(&assigned),
        obj_labels,
      )?;
    }
    Err(err) => {
      f.write_fmt(format_args!("Result: failure\n{}", err))?;
    }
  }
  Ok(())
}

// don't allocate memory, just print possible mem assignments
#[allow(dead_code)]
pub fn debug_print_possible_memory_type_assignment<const P: usize, const S: usize>(
  device: &Device,
  physical_device: &PhysicalDevice,
  mem_props: [vk::MemoryPropertyFlags; P],
  objs: [&dyn MemoryBound; S],
  obj_labels: Option<[&'static str; S]>,
) -> fmt::Result {
  let mut output = String::new();
  let mem_types = physical_device.memory_types();
  let obj_reqs = unsafe { objs.map(|obj| obj.get_memory_requirements(device)) };

  output.write_fmt(format_args!(
    "\nFinding possible allocation configuration for {} objects:\n",
    objs.len()
  ))?;

  let result =
    assign_memory_type_indexes_to_objects_for_allocation(UnassignedToMemoryObjectsData {
      mem_types,
      mem_props,
      obj_reqs,
      obj_labels,
    });
  let labels = match &obj_labels {
    Some(labels_arr) => Some(labels_arr.as_slice()),
    None => None,
  };
  display_mem_assignment_result(
    &mut output,
    result,
    mem_types,
    &mem_props,
    &obj_reqs,
    labels,
  )
  .unwrap();

  log::debug!("{}", output);
  Ok(())
}
