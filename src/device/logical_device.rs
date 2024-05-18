use ash::vk::{self};
use std::{
  marker::PhantomData,
  os::raw::{c_char, c_void},
  ptr::{self},
};

use crate::REQUIRED_DEVICE_EXTENSIONS;

use super::{PhysicalDevice, Queues};

pub fn create_logical_device(
  instance: &ash::Instance,
  physical_device: &PhysicalDevice,
) -> Result<(ash::Device, Queues), vk::Result> {
  let queue_create_infos = Queues::get_queue_create_infos(&physical_device.queue_families);

  let device_extensions_ptrs: Vec<*const c_char> = REQUIRED_DEVICE_EXTENSIONS
    .iter()
    .map(|s| s.as_ptr())
    .collect();

  // enabled features
  let features10 = vk::PhysicalDeviceFeatures::default();
  let mut features12 = vk::PhysicalDeviceVulkan12Features {
    timeline_semaphore: vk::TRUE,
    ..Default::default()
  };
  let mut features13 = vk::PhysicalDeviceVulkan13Features {
    synchronization2: vk::TRUE,
    ..Default::default()
  };

  features12.p_next = &mut features13 as *mut vk::PhysicalDeviceVulkan13Features as *mut c_void;
  features13.p_next = ptr::null_mut();

  #[allow(deprecated)]
  let create_info = vk::DeviceCreateInfo {
    s_type: vk::StructureType::DEVICE_CREATE_INFO,
    p_queue_create_infos: queue_create_infos.as_ptr(),
    queue_create_info_count: queue_create_infos.len() as u32,
    p_enabled_features: &features10,
    p_next: &features12 as *const vk::PhysicalDeviceVulkan12Features as *const c_void,
    pp_enabled_layer_names: ptr::null(), // deprecated
    enabled_layer_count: 0,              // deprecated
    pp_enabled_extension_names: device_extensions_ptrs.as_ptr(),
    enabled_extension_count: device_extensions_ptrs.len() as u32,
    flags: vk::DeviceCreateFlags::empty(),
    _marker: PhantomData,
  };
  log::debug!("Creating logical device");
  let device: ash::Device =
    unsafe { instance.create_device(**physical_device, &create_info, None)? };

  log::debug!("Retrieving queues");
  let queues = unsafe { Queues::retrieve(&device, &physical_device.queue_families) };

  Ok((device, queues))
}
