use std::ops::BitOr;

use ash::vk;

use crate::utility::c_char_array_to_string;

use super::{get_extended_properties, select_physical_device};

use super::QueueFamilies;

// in order to not query physical device info multiple times, this struct saves the additional information
pub struct PhysicalDevice {
  pub vk_device: vk::PhysicalDevice,
  pub queue_families: QueueFamilies,
  mem_properties: vk::PhysicalDeviceMemoryProperties,
  max_memory_allocation_size: vk::DeviceSize,
}

impl PhysicalDevice {
  pub unsafe fn select(instance: &ash::Instance) -> PhysicalDevice {
    let (physical_device, queue_families) =
      select_physical_device(instance).expect("No supported physical device available");

    let (properties, properties11) = get_extended_properties(instance, physical_device);
    let mem_properties = instance.get_physical_device_memory_properties(physical_device);
    let queue_family_properties =
      instance.get_physical_device_queue_family_properties(physical_device);

    log::info!(
      "Using physical device \"{}\"",
      c_char_array_to_string(&properties.device_name)
    );
    print_queue_families_debug_info(&queue_family_properties);
    print_device_memory_debug_info(&mem_properties);

    PhysicalDevice {
      vk_device: physical_device,
      mem_properties,
      queue_families,
      max_memory_allocation_size: properties11.max_memory_allocation_size,
    }
  }

  pub fn find_memory_type(
    &self,
    required_memory_type_bits: u32,
    required_properties: vk::MemoryPropertyFlags,
  ) -> Result<u32, ()> {
    for (i, memory_type) in self.mem_properties.memory_types.iter().enumerate() {
      let valid_type = required_memory_type_bits & (1 << i) > 0;
      if valid_type && memory_type.property_flags.contains(required_properties) {
        return Ok(i as u32);
      }
    }

    Err(())
  }

  // Tries to find optimal memory type. If it fails, tries to find a memory type with only
  // required flags
  pub fn find_optimal_memory_type(
    &self,
    required_memory_type_bits: u32,
    required_properties: vk::MemoryPropertyFlags,
    optional_properties: vk::MemoryPropertyFlags,
  ) -> Result<u32, ()> {
    self
      .find_memory_type(
        required_memory_type_bits,
        required_properties.bitor(optional_properties),
      )
      .or_else(|()| self.find_memory_type(required_memory_type_bits, required_properties))
  }

  pub fn get_memory_type(&self, type_i: u32) -> vk::MemoryType {
    self.mem_properties.memory_types[type_i as usize]
  }

  pub fn get_memory_type_heap(&self, type_i: u32) -> vk::MemoryHeap {
    let mem_type = self.get_memory_type(type_i);
    self.mem_properties.memory_heaps[mem_type.heap_index as usize]
  }

  pub fn get_max_memory_allocation_size(&self) -> vk::DeviceSize {
    self.max_memory_allocation_size
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
