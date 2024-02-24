mod logical_device;
mod physical_device;
mod queues;
mod vendor;

pub use logical_device::create_logical_device;
pub use physical_device::PhysicalDevice;
pub use queues::{QueueFamilies, Queues};

use self::vendor::Vendor;
use std::{
  ffi::c_void,
  mem::MaybeUninit,
  ptr::{self, addr_of_mut},
};

use ash::vk;

use crate::{
  utility::{self, c_char_array_to_string},
  REQUIRED_DEVICE_EXTENSIONS, TARGET_API_VERSION,
};

fn log_device_properties(properties: &vk::PhysicalDeviceProperties) {
  let vendor = Vendor::from_id(properties.vendor_id);
  let driver_version = vendor.parse_driver_version(properties.driver_version);

  log::info!(
    "\nFound physical device \"{}\":
      API Version: {},
      Vendor: {},
      Driver Version: {},
      ID: {},
      Type: {},",
    c_char_array_to_string(&properties.device_name),
    utility::parse_vulkan_api_version(properties.api_version),
    vendor.to_string(),
    driver_version,
    properties.device_id,
    match properties.device_type {
      vk::PhysicalDeviceType::INTEGRATED_GPU => "Integrated GPU",
      vk::PhysicalDeviceType::DISCRETE_GPU => "Discrete GPU",
      vk::PhysicalDeviceType::VIRTUAL_GPU => "Virtual GPU",
      vk::PhysicalDeviceType::CPU => "CPU",
      _ => "Unknown",
    },
  );
}

fn check_extension_support(instance: &ash::Instance, device: vk::PhysicalDevice) -> bool {
  let properties = unsafe {
    instance
      .enumerate_device_extension_properties(device)
      .expect("Failed to get device extension properties")
  };

  let mut available: Vec<String> = properties
    .into_iter()
    .map(|prop| utility::c_char_array_to_string(&prop.extension_name))
    .collect();

  utility::not_in_slice(
    available.as_mut_slice(),
    &mut REQUIRED_DEVICE_EXTENSIONS.iter(),
    |av, req| av.as_str().cmp(req.to_str().unwrap()),
  )
  .is_empty()
}

unsafe fn select_physical_device(
  instance: &ash::Instance,
) -> Option<(vk::PhysicalDevice, QueueFamilies)> {
  instance
    .enumerate_physical_devices()
    .expect("Failed to enumerate physical devices")
    .into_iter()
    .filter(|&physical_device| {
      // Filter devices that are strictly not supported
      // Check for any features or limits required by the application

      let properties = instance.get_physical_device_properties(physical_device);
      log_device_properties(&properties);
      let (_features10, _features11, features12, features13) =
        get_extended_features(instance, physical_device);

      if properties.api_version < TARGET_API_VERSION {
        log::info!(
          "Skipped physical device: Device API version is less than targeted by the application"
        );
        return false;
      }

      // check if device supports all required extensions
      if !check_extension_support(instance, physical_device) {
        log::info!("Skipped physical device: Device does not support all required extensions");
        return false;
      }

      if features12.timeline_semaphore != vk::TRUE {
        log::warn!("Skipped physical device: Device does not support timeline semaphores");
        return false;
      }

      if features13.synchronization2 != vk::TRUE {
        log::warn!("Skipped physical device: Device does not support synchronization features");
        return false;
      }

      true
    })
    .filter_map(|physical_device| {
      // filter devices that do not have required queue families
      match QueueFamilies::get_from_physical_device(instance, physical_device) {
        Err(()) => {
          log::info!("Skipped physical device: Device does not contain required queue families");
          None
        }
        Ok(families) => Some((physical_device, families)),
      }
    })
    .min_by_key(|(physical_device, families)| {
      // Assign a score to each device and select the best one available
      // A full application may use multiple metrics like limits, queue families and even the
      // device id to rank each device that a user can have

      let queue_family_importance = 3;
      let device_score_importance = 0;

      // rank devices by number of specialized queue families
      let queue_score = if families.compute.is_some() { 0 } else { 1 }
        + if families.transfer.is_some() { 0 } else { 1 };

      // rank devices by commonly most powerful device type
      let device_score = match instance
        .get_physical_device_properties(*physical_device)
        .device_type
      {
        vk::PhysicalDeviceType::DISCRETE_GPU => 0,
        vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
        vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
        vk::PhysicalDeviceType::CPU => 3,
        vk::PhysicalDeviceType::OTHER => 4,
        _ => 5,
      };

      (queue_score << queue_family_importance) + (device_score << device_score_importance)
    })
}

fn get_extended_properties(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
) -> (
  vk::PhysicalDeviceProperties,
  vk::PhysicalDeviceVulkan11Properties,
) {
  // going c style (see https://doc.rust-lang.org/std/mem/union.MaybeUninit.html)
  let mut props10: MaybeUninit<vk::PhysicalDeviceProperties2> = MaybeUninit::uninit();
  let mut props11: MaybeUninit<vk::PhysicalDeviceVulkan11Properties> = MaybeUninit::uninit();

  let props10_ptr = props10.as_mut_ptr();
  let props11_ptr = props11.as_mut_ptr();

  unsafe {
    addr_of_mut!((*props10_ptr).s_type).write(vk::StructureType::PHYSICAL_DEVICE_PROPERTIES_2);
    addr_of_mut!((*props11_ptr).s_type)
      .write(vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_1_PROPERTIES);

    // requesting for Vulkan11Properties
    addr_of_mut!((*props10_ptr).p_next).write(props11_ptr as *mut c_void);
    addr_of_mut!((*props11_ptr).p_next).write(ptr::null_mut::<c_void>());

    instance.get_physical_device_properties2(physical_device, props10_ptr.as_mut().unwrap());

    (props10.assume_init().properties, props11.assume_init())
  }
}

fn get_extended_features(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
) -> (
  vk::PhysicalDeviceFeatures,
  vk::PhysicalDeviceVulkan11Features,
  vk::PhysicalDeviceVulkan12Features,
  vk::PhysicalDeviceVulkan13Features,
) {
  let mut features10: MaybeUninit<vk::PhysicalDeviceFeatures2> = MaybeUninit::uninit();
  let mut features11: MaybeUninit<vk::PhysicalDeviceVulkan11Features> = MaybeUninit::uninit();
  let mut features12: MaybeUninit<vk::PhysicalDeviceVulkan12Features> = MaybeUninit::uninit();
  let mut features13: MaybeUninit<vk::PhysicalDeviceVulkan13Features> = MaybeUninit::uninit();

  let features10_ptr = features10.as_mut_ptr();
  let features11_ptr = features11.as_mut_ptr();
  let features12_ptr = features12.as_mut_ptr();
  let features13_ptr = features13.as_mut_ptr();

  unsafe {
    addr_of_mut!((*features10_ptr).s_type).write(vk::StructureType::PHYSICAL_DEVICE_FEATURES_2);
    addr_of_mut!((*features11_ptr).s_type)
      .write(vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_1_FEATURES);
    addr_of_mut!((*features12_ptr).s_type)
      .write(vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_2_FEATURES);
    addr_of_mut!((*features13_ptr).s_type)
      .write(vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_3_FEATURES);

    addr_of_mut!((*features10_ptr).p_next).write(features11_ptr as *mut c_void);
    addr_of_mut!((*features11_ptr).p_next).write(features12_ptr as *mut c_void);
    addr_of_mut!((*features12_ptr).p_next).write(features13_ptr as *mut c_void);
    addr_of_mut!((*features13_ptr).p_next).write(ptr::null_mut::<c_void>());

    instance.get_physical_device_features2(physical_device, features10_ptr.as_mut().unwrap());

    (
      features10.assume_init().features,
      features11.assume_init(),
      features12.assume_init(),
      features13.assume_init(),
    )
  }
}
