use ash::vk::{self};
use std::{os::raw::c_char, ptr};

use crate::render::REQUIRED_DEVICE_EXTENSIONS;

use super::{PhysicalDevice, Queues};

pub fn create_logical_device(
  instance: &ash::Instance,
  physical_device: &PhysicalDevice,
) -> (ash::Device, Queues) {
  let queue_create_infos = Queues::get_queue_create_infos(&physical_device.queue_families);

  let device_extensions_pointers: Vec<*const c_char> = REQUIRED_DEVICE_EXTENSIONS
    .iter()
    .map(|s| s.as_ptr() as *const i8)
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
    pp_enabled_layer_names: ptr::null(), // deprecated
    enabled_layer_count: 0,              // deprecated
    pp_enabled_extension_names: device_extensions_pointers.as_ptr(),
    enabled_extension_count: device_extensions_pointers.len() as u32,
    flags: vk::DeviceCreateFlags::empty(),
  };
  log::info!("Creating logical device");
  let device: ash::Device = unsafe {
    instance
      .create_device(**physical_device, &create_info, None)
      .expect("Failed to create logical device")
  };

  log::debug!("Retrieving queues");
  let queues = unsafe { Queues::retrieve(&device, &physical_device.queue_families) };

  (device, queues)
}
