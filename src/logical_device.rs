use ash::vk::{self, DeviceQueueCreateInfo};
use std::{cmp::min, os::raw::c_char, ptr};

use crate::{physical_device::QueueFamilies, REQUIRED_DEVICE_EXTENSIONS};

const MAX_FAMILY_COUNT: usize = 3;

// Compute and transfer queues will be queried from the graphics queue family if
// their own families do not exist
pub struct Queues {
  pub graphics: vk::Queue,
  pub compute: vk::Queue,
  pub transfer: vk::Queue,
}

fn get_queue_create_info(
  index: u32,
  count: u32,
  priorities_ptr: *const f32,
) -> vk::DeviceQueueCreateInfo {
  vk::DeviceQueueCreateInfo {
    s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
    queue_family_index: index,
    queue_count: count,
    p_queue_priorities: priorities_ptr,
    p_next: ptr::null(),
    flags: vk::DeviceQueueCreateFlags::empty(),
  }
}

fn get_queue_create_infos(
  families: &QueueFamilies,
  priorities_ptr: *const f32,
) -> Vec<DeviceQueueCreateInfo> {
  let mut queues_create_infos = Vec::with_capacity(MAX_FAMILY_COUNT);

  // add optional queues
  for optional_family in [&families.compute, &families.transfer] {
    if let Some(family) = optional_family {
      queues_create_infos.push(get_queue_create_info(family.index, 1, priorities_ptr));
    }
  }

  // add graphics queues, these will substitute not available queues
  queues_create_infos.push(get_queue_create_info(
    families.graphics.index,
    min(
      // always watch out for limits
      families.graphics.queue_count,
      // request remaining needed queues
      (MAX_FAMILY_COUNT - queues_create_infos.len()) as u32,
    ),
    priorities_ptr,
  ));

  queues_create_infos
}

unsafe fn retrieve_queues(device: &ash::Device, families: &QueueFamilies) -> Queues {
  let mut graphics_i = 0;
  let mut get_next_graphics_queue = || {
    let queue = device.get_device_queue(families.graphics.index, graphics_i);
    if graphics_i < families.graphics.queue_count {
      graphics_i += 1;
    }
    queue
  };

  let graphics = get_next_graphics_queue();
  let compute = match &families.compute {
    Some(family) => device.get_device_queue(family.index, 0),
    None => get_next_graphics_queue(),
  };
  let transfer = match &families.transfer {
    Some(family) => device.get_device_queue(family.index, 0),
    None => get_next_graphics_queue(),
  };

  Queues {
    graphics,
    compute,
    transfer,
  }
}

pub fn create_logical_device(
  instance: &ash::Instance,
  physical_device: &vk::PhysicalDevice,
  families: &QueueFamilies,
) -> (ash::Device, Queues) {
  // In this case priorities can matter if the compute or transfer queues get queried from the
  // graphics family
  // Queue priority is managed by the driver which chooses how to interpret them
  // This may lead to queue starvation so use with caution
  // The possible values are between 0.0 (low priority) and 1.0 (high priority)
  let queue_priorities = [0.5_f32; MAX_FAMILY_COUNT];
  let queue_priorities_ptr = queue_priorities.as_ptr();
  // this contains the priorities pointer so it need to be valid until end of scope
  let queue_create_infos = get_queue_create_infos(families, queue_priorities_ptr);

  let device_extensions_pointers: Vec<*const c_char> = REQUIRED_DEVICE_EXTENSIONS
    .iter()
    .map(|s| s.as_ptr())
    .collect();

  // in this case there are no features
  let features = vk::PhysicalDeviceFeatures::default();

  // pp_enabled_layer_names are deprecated however they are still required in struct initialization
  #[allow(deprecated)]
  let create_info = vk::DeviceCreateInfo {
    s_type: vk::StructureType::DEVICE_CREATE_INFO,
    p_queue_create_infos: queue_create_infos.as_ptr(),
    queue_create_info_count: queue_create_infos.len() as u32,
    p_enabled_features: &features,
    p_next: ptr::null(),
    pp_enabled_layer_names: ptr::null(),
    enabled_layer_count: 0,
    pp_enabled_extension_names: device_extensions_pointers.as_ptr(),
    enabled_extension_count: device_extensions_pointers.len() as u32,
    flags: vk::DeviceCreateFlags::empty(),
  };

  log::info!("Creating logical device");
  let device: ash::Device = unsafe {
    instance
      .create_device(*physical_device, &create_info, None)
      .expect("Failed to create logical device")
  };
  log::debug!("Retrieving queues");
  let queues = unsafe { retrieve_queues(&device, families) };

  (device, queues)
}
