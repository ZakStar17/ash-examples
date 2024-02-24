use std::ops::Deref;

use ash::vk;

use crate::utility;

use super::{get_extended_properties, select_physical_device};

use super::QueueFamilies;

// Saves physical device additional information in order to not query it multiple times
pub struct PhysicalDevice {
  inner: vk::PhysicalDevice,
  pub queue_families: QueueFamilies,
}

impl Deref for PhysicalDevice {
  type Target = vk::PhysicalDevice;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl PhysicalDevice {
  pub unsafe fn select(instance: &ash::Instance) -> PhysicalDevice {
    let (physical_device, queue_families) =
      select_physical_device(instance).expect("No supported physical device available");

    let (properties, _properties11) = get_extended_properties(instance, physical_device);
    let mem_properties = instance.get_physical_device_memory_properties(physical_device);
    let queue_family_properties =
      instance.get_physical_device_queue_family_properties(physical_device);

    log::info!(
      "Using physical device \"{}\"",
      utility::c_char_array_to_string(&properties.device_name)
    );
    print_queue_families_debug_info(&queue_family_properties);
    print_device_memory_debug_info(&mem_properties);

    PhysicalDevice {
      inner: physical_device,
      queue_families,
    }
  }
}

fn print_queue_families_debug_info(properties: &Vec<vk::QueueFamilyProperties>) {
  log::debug!("Queue family properties: {:#?}", properties);
}

fn print_device_memory_debug_info(mem_properties: &vk::PhysicalDeviceMemoryProperties) {
  log::debug!("Available memory heaps:");
  for heap_i in 0..mem_properties.memory_heap_count {
    let heap = mem_properties.memory_heaps[heap_i as usize];
    let heap_flags = if heap.flags.is_empty() {
      String::from("no heap flags")
    } else {
      format!("heap flags [{:?}]", heap.flags)
    };

    log::debug!(
      "    {} -> {}mb with {} and attributed memory types:",
      heap_i,
      heap.size / 1000000,
      heap_flags
    );
    for type_i in 0..mem_properties.memory_type_count {
      let mem_type = mem_properties.memory_types[type_i as usize];
      if mem_type.heap_index != heap_i {
        continue;
      }

      let flags = mem_type.property_flags;
      log::debug!(
        "        {} -> {}",
        type_i,
        if flags.is_empty() {
          "<no flags>".to_owned()
        } else {
          format!("[{:?}]", flags)
        }
      );
    }
  }
}
