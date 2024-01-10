use ash::vk;
use log::info;
use std::{cmp::min, ffi::CString, os::raw::c_char, ptr};

use crate::physical_device::QueueFamilies;

// The application will only use one of each queue type
// Compute and transfer queues can be queried from the graphics queue family if
// their own families do not exist
pub struct Queues {
  pub graphics: vk::Queue,
  pub compute: vk::Queue,
  pub transfer: vk::Queue,
}

pub fn create_logical_device(
  instance: &ash::Instance,
  physical_device: &vk::PhysicalDevice,
  device_extensions: &[String],
  families: &QueueFamilies,
  #[cfg(feature = "vulkan_vl")] vl_pointers: &Vec<*const c_char>,
) -> (ash::Device, Queues) {
  let mut queues_create_infos = Vec::with_capacity(3);

  // These priorities will only matter if the compute or transfer queues get queried from the
  // graphics family
  // Queue priority is completely managed by the driver, which may sometimes starve low priority
  // queues or otherwise do nothing, so use with caution
  // The possible values are between 0.0 (low priority) and 1.0 (high priority)
  let queue_priorities = [1.0_f32; 3]; // should remain alive trough device creation
  let queue_priorities_ptr = queue_priorities.as_ptr();

  // add graphics queue create info
  queues_create_infos.push(vk::DeviceQueueCreateInfo {
    s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
    queue_family_index: families.graphics.index,
    queue_count: min(
      families.graphics.queue_count,
      // 1 + number of non unique families
      4 - families.unique_indices.len() as u32,
    ),
    p_queue_priorities: queue_priorities_ptr,
    p_next: ptr::null(),
    flags: vk::DeviceQueueCreateFlags::empty(),
  });

  // set transfer or compute queue family to graphics if their family is None
  for opt in [&families.compute, &families.transfer] {
    if let Some(f) = opt {
      queues_create_infos.push(vk::DeviceQueueCreateInfo {
        s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
        queue_family_index: f.index,
        queue_count: 1,
        p_queue_priorities: queue_priorities_ptr,
        p_next: ptr::null(),
        flags: vk::DeviceQueueCreateFlags::empty(),
      });
    }
  }

  let device_extensions_c: Vec<CString> = device_extensions
    .iter()
    .map(|s| CString::new(s.as_bytes()).expect("Invalid device extension"))
    .collect();
  let device_extensions_pointers: Vec<*const c_char> =
    device_extensions_c.iter().map(|s| s.as_ptr()).collect();

  // use no features
  // tutorial note: I may change this later depending on what I choose to add
  let features = vk::PhysicalDeviceFeatures::default();

  #[allow(unused_mut)]
  #[allow(deprecated)]
  // pp_enabled_layer_names are deprecated however they are still required in struct initialization
  let mut create_info = vk::DeviceCreateInfo {
    s_type: vk::StructureType::DEVICE_CREATE_INFO,
    p_queue_create_infos: queues_create_infos.as_ptr(),
    queue_create_info_count: queues_create_infos.len() as u32,
    p_enabled_features: &features,
    p_next: ptr::null(),
    pp_enabled_layer_names: ptr::null(),
    enabled_layer_count: 0,
    pp_enabled_extension_names: device_extensions_pointers.as_ptr(),
    enabled_extension_count: device_extensions_pointers.len() as u32,
    flags: vk::DeviceCreateFlags::empty(),
  };

  // add validation layers if enabled
  #[cfg(feature = "vulkan_vl")]
  #[allow(deprecated)]
  {
    create_info.pp_enabled_layer_names = vl_pointers.as_ptr();
    create_info.enabled_layer_count = vl_pointers.len() as u32;
  }

  info!("Creating logical device");
  let device: ash::Device = unsafe {
    instance
      .create_device(*physical_device, &create_info, None)
      .expect("Failed to create logical device")
  };

  // compute and transfer will default to using graphics queues (preferably different from each
  // other) if their specialized queues are not available
  let queues = unsafe {
    let graphics = device.get_device_queue(families.graphics.index, 0);

    let mut next_graphics_queue_i = 1;
    let compute = if let Some(compute) = &families.compute {
      device.get_device_queue(compute.index, 0)
    } else {
      let queue = device.get_device_queue(
        families.graphics.index,
        min(next_graphics_queue_i, families.graphics.queue_count - 1),
      );
      next_graphics_queue_i += 1;
      queue
    };

    let transfer = if let Some(transfer) = &families.transfer {
      device.get_device_queue(transfer.index, 0)
    } else {
      device.get_device_queue(
        families.graphics.index,
        min(next_graphics_queue_i, families.graphics.queue_count - 1),
      )
    };

    Queues { graphics, compute, transfer }
  };

  (device, queues)
}
