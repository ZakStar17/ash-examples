use std::{
  ffi::CStr,
  fmt::{self, Write},
  ops::Deref,
};

use ash::vk;

use super::select_physical_device;

use super::QueueFamilies;

// Saves physical device additional information in order to not query it multiple times
pub struct PhysicalDevice {
  inner: vk::PhysicalDevice,
  pub queue_families: QueueFamilies,
  pub queue_family_properties: Box<[vk::QueueFamilyProperties]>,
}

impl Deref for PhysicalDevice {
  type Target = vk::PhysicalDevice;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl PhysicalDevice {
  pub unsafe fn select(instance: &ash::Instance) -> Result<Option<PhysicalDevice>, vk::Result> {
    match select_physical_device(instance)? {
      Some((physical_device, properties, _features, queue_families)) => {
        let mem_properties = instance.get_physical_device_memory_properties(physical_device);
        let queue_family_properties = instance
          .get_physical_device_queue_family_properties(physical_device)
          .into_boxed_slice();

        log::info!(
          "Using physical device {:?}",
          unsafe { CStr::from_ptr(properties.p10.device_name.as_ptr()) }, // expected to be a valid cstr
        );
        print_queue_families_debug_info(&queue_family_properties);
        debug_print_device_memory_info(&mem_properties).unwrap();

        Ok(Some(PhysicalDevice {
          inner: physical_device,
          queue_families,
          queue_family_properties,
        }))
      }
      None => Ok(None),
    }
  }
}

fn print_queue_families_debug_info(properties: &[vk::QueueFamilyProperties]) {
  log::debug!("Physical device queue family properties: {:#?}", properties);
}

fn debug_print_device_memory_info(
  mem_properties: &vk::PhysicalDeviceMemoryProperties,
) -> fmt::Result {
  let mut output = String::new();

  output.write_fmt(format_args!(
    "\nAvailable memory heaps: ({} heaps, {} memory types)",
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
      "\n    {} -> {}MiB with {} and attributed memory types:",
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
        "\n        {} -> {}",
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
