use std::ops::Deref;

use ash::vk;

use crate::utility::c_char_array_to_string;

use super::select_physical_device;

use super::{PhysicalDeviceProperties, QueueFamilies};

// Saves physical device additional information in order to not query it multiple times
pub struct PhysicalDevice {
  inner: vk::PhysicalDevice,
  pub queue_families: QueueFamilies,
  pub mem_properties: vk::PhysicalDeviceMemoryProperties,
  pub properties: PhysicalDeviceProperties,
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
        let queue_family_properties =
          instance.get_physical_device_queue_family_properties(physical_device);

        log::info!(
          "Using physical device \"{}\"",
          c_char_array_to_string(&properties.p10.device_name)
        );
        print_queue_families_debug_info(&queue_family_properties);
        print_device_memory_debug_info(&mem_properties);

        log::info!(
          "Using physical device \"{}\"",
          c_char_array_to_string(&properties.p10.device_name)
        );
        print_queue_families_debug_info(&queue_family_properties);
        print_device_memory_debug_info(&mem_properties);

        Ok(Some(PhysicalDevice {
          inner: physical_device,
          queue_families,
          mem_properties,
          properties,
        }))
      }
      None => Ok(None),
    }
  }

  pub fn memory_type_heap(&self, type_i: usize) -> vk::MemoryHeap {
    self.mem_properties.memory_heaps[self.mem_properties.memory_types[type_i].heap_index as usize]
  }
}

pub struct MemoryTypesIterator<'a> {
  valid_types_bitmask: u32,
  i: usize,
  required_properties: vk::MemoryPropertyFlags,
  types: &'a [vk::MemoryType; vk::MAX_MEMORY_TYPES],
  types_count: usize,
}

impl<'a> Iterator for MemoryTypesIterator<'a> {
  type Item = (usize, vk::MemoryType);

  fn next(&mut self) -> Option<Self::Item> {
    loop {
      if self.i >= self.types_count {
        return None;
      }
      let valid_bit = self.valid_types_bitmask & (1 << self.i) > 0;
      if valid_bit
        && self.types[self.i]
          .property_flags
          .contains(self.required_properties)
      {
        let item = Some((self.i, self.types[self.i]));
        self.i += 1;
        return item;
      }
      self.i += 1;
    }
  }
}

impl<'a> MemoryTypesIterator<'a> {
  pub fn new(
    physical_device: &'a PhysicalDevice,
    valid_types_bitmask: u32,
    memory_properties: vk::MemoryPropertyFlags,
  ) -> Self {
    Self {
      valid_types_bitmask,
      i: 0,
      required_properties: memory_properties,
      types: &physical_device.mem_properties.memory_types,
      types_count: physical_device.mem_properties.memory_type_count as usize,
    }
  }
}

// filters memory types by unique heaps
pub struct UniqueHeapMemoryTypesIterator<'a> {
  iter: MemoryTypesIterator<'a>,
  iterated_heaps: [bool; vk::MAX_MEMORY_HEAPS],
}

impl<'a> Iterator for UniqueHeapMemoryTypesIterator<'a> {
  type Item = <MemoryTypesIterator<'a> as Iterator>::Item;

  fn next(&mut self) -> Option<Self::Item> {
    while let Some(next) = self.iter.next() {
      if !self.iterated_heaps[next.1.heap_index as usize] {
        self.iterated_heaps[next.1.heap_index as usize] = true;
        return Some(next);
      }
    }
    None
  }
}

impl<'a> UniqueHeapMemoryTypesIterator<'a> {
  pub fn new(
    physical_device: &'a PhysicalDevice,
    valid_types_bitmask: u32,
    memory_properties: vk::MemoryPropertyFlags,
  ) -> Self {
    Self {
      iter: MemoryTypesIterator::new(physical_device, valid_types_bitmask, memory_properties),
      iterated_heaps: [false; vk::MAX_MEMORY_HEAPS],
    }
  }
}

impl PhysicalDevice {
  pub fn iterate_memory_types_with_unique_heaps(
    &self,
    valid_types_bitmask: u32,
    memory_properties: vk::MemoryPropertyFlags,
  ) -> UniqueHeapMemoryTypesIterator {
    UniqueHeapMemoryTypesIterator::new(self, valid_types_bitmask, memory_properties)
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
