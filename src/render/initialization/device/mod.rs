mod logical_device;
mod physical_device;
mod queues;
mod vendor;

use std::{
  ffi::{c_void, CStr},
  mem::MaybeUninit,
  ptr::{self, addr_of_mut},
};

use ash::vk;
pub use logical_device::create_logical_device;
pub use physical_device::PhysicalDevice;
pub use queues::{QueueFamilies, Queues};

use self::vendor::Vendor;
use crate::{
  render::{
    errors::OutOfMemoryError, initialization::device::queues::QueueFamilyError,
    REQUIRED_DEVICE_EXTENSIONS, TARGET_API_VERSION,
  },
  utility::{self, i8_array_as_cstr},
};

use super::{Surface, SurfaceError};

fn log_device_properties(properties: &vk::PhysicalDeviceProperties) {
  let vendor = Vendor::from_id(properties.vendor_id);
  let driver_version = vendor.parse_driver_version(properties.driver_version);

  log::info!(
    "\nFound physical device \"{:?}\":
      API Version: {},
      Vendor: {},
      Driver Version: {},
      ID: {},
      Type: {},",
    unsafe { CStr::from_ptr(properties.device_name.as_ptr()) }, // expected to be a valid cstr
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

fn supports_required_extensions(
  instance: &ash::Instance,
  device: vk::PhysicalDevice,
) -> Result<bool, OutOfMemoryError> {
  let properties = unsafe { instance.enumerate_device_extension_properties(device)? };

  for req in REQUIRED_DEVICE_EXTENSIONS {
    if !properties
      .iter()
      .any(|props| unsafe { i8_array_as_cstr(&props.extension_name) }.unwrap() == req)
    {
      return Ok(false);
    }
  }

  Ok(true)
}

fn supports_swapchain(device: vk::PhysicalDevice, surface: &Surface) -> Result<bool, SurfaceError> {
  let formats = unsafe { surface.get_formats(device) }?;
  let present_modes = unsafe { surface.get_present_modes(device) }?;

  Ok(!formats.is_empty() && !present_modes.is_empty())
}

fn check_physical_device_capabilities(
  instance: &ash::Instance,
  surface: &Surface,
  physical_device: vk::PhysicalDevice,
  properties: &PhysicalDeviceProperties,
  features: &PhysicalDeviceFeatures,
) -> Result<bool, SurfaceError> {
  // Filter devices that are strictly not supported
  // Check for any features or limits required by the application

  if properties.p10.api_version < TARGET_API_VERSION {
    log::info!(
      "Skipped physical device: Device API version is less than targeted by the application"
    );
    return Ok(false);
  }

  if !supports_required_extensions(instance, physical_device).map_err(SurfaceError::OutOfMemory)? {
    log::info!("Skipped physical device: Device does not support all required extensions");
    return Ok(false);
  }

  if !supports_swapchain(physical_device, surface)? {
    log::warn!("Skipped physical device: Device does not support swapchain");
    return Ok(false);
  }

  if features.f12.timeline_semaphore != vk::TRUE {
    log::warn!("Skipped physical device: Device does not support timeline semaphores");
    return Ok(false);
  }

  if features.f13.synchronization2 != vk::TRUE {
    log::warn!("Skipped physical device: Device does not support synchronization features");
    return Ok(false);
  }

  Ok(true)
}

unsafe fn select_physical_device<'a>(
  instance: &'a ash::Instance,
  surface: &'a Surface,
) -> Result<
  Option<(
    vk::PhysicalDevice,
    PhysicalDeviceProperties<'a>,
    PhysicalDeviceFeatures<'a>,
    QueueFamilies,
  )>,
  vk::Result,
> {
  Ok(
    instance
      .enumerate_physical_devices()?
      .into_iter()
      .filter_map(|physical_device| {
        // Filter devices that are strictly not supported
        // Check for any features or limits required by the application

        let properties = get_extended_properties(instance, physical_device);
        log_device_properties(&properties.p10);
        let features = get_extended_features(instance, physical_device);

        match check_physical_device_capabilities(
          instance,
          surface,
          physical_device,
          &properties,
          &features,
        ) {
          Ok(all_good) => {
            if all_good {
              Some((physical_device, properties, features))
            } else {
              None
            }
          }
          Err(err) => {
            log::error!("Device selection error: {:?}", err);
            None
          }
        }
      })
      .filter_map(|(physical_device, properties, features)| {
        // filter devices that do not have required queue families
        match QueueFamilies::get_from_physical_device(instance, physical_device, surface) {
          Err(err) => {
            match err {
              QueueFamilyError::DoesNotSupportRequiredQueueFamilies => log::info!(
                "Skipped physical device: Device does not contain required queue families"
              ),
              QueueFamilyError::SurfaceError(err) => log::error!(
                "Device selection error during queue family retrieval: {:?}",
                err
              ),
            }
            None
          }
          Ok(families) => Some((physical_device, properties, features, families)),
        }
      })
      .min_by_key(|(physical_device, _properties, _features, families)| {
        // Assign a score to each device and select the best one available
        // A full application may use multiple metrics like limits, queue families and even the
        // device id to rank each device that a user can have

        let queue_family_importance = 3;
        let device_score_importance = 0;

        // rank devices by number of specialized queue families
        let queue_score = if families.transfer.is_some() { 0 } else { 1 };

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
      }),
  )
}

#[allow(unused)]
struct PhysicalDeviceProperties<'a> {
  pub p10: vk::PhysicalDeviceProperties,
  pub p11: vk::PhysicalDeviceVulkan11Properties<'a>,
  pub p12: vk::PhysicalDeviceVulkan12Properties<'a>,
  pub p13: vk::PhysicalDeviceVulkan13Properties<'a>,
}

fn get_extended_properties(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
) -> PhysicalDeviceProperties {
  // see https://doc.rust-lang.org/std/mem/union.MaybeUninit.html
  let mut props10: MaybeUninit<vk::PhysicalDeviceProperties2> = MaybeUninit::uninit();
  let mut props11: MaybeUninit<vk::PhysicalDeviceVulkan11Properties> = MaybeUninit::uninit();
  let mut props12: MaybeUninit<vk::PhysicalDeviceVulkan12Properties> = MaybeUninit::uninit();
  let mut props13: MaybeUninit<vk::PhysicalDeviceVulkan13Properties> = MaybeUninit::uninit();

  let props10_ptr = props10.as_mut_ptr();
  let props11_ptr = props11.as_mut_ptr();
  let props12_ptr = props12.as_mut_ptr();
  let props13_ptr = props13.as_mut_ptr();

  unsafe {
    addr_of_mut!((*props10_ptr).s_type).write(vk::StructureType::PHYSICAL_DEVICE_PROPERTIES_2);
    addr_of_mut!((*props11_ptr).s_type)
      .write(vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_1_PROPERTIES);
    addr_of_mut!((*props12_ptr).s_type)
      .write(vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_2_PROPERTIES);
    addr_of_mut!((*props13_ptr).s_type)
      .write(vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_3_PROPERTIES);

    addr_of_mut!((*props10_ptr).p_next).write(props11_ptr as *mut c_void);
    addr_of_mut!((*props11_ptr).p_next).write(props12_ptr as *mut c_void);
    addr_of_mut!((*props12_ptr).p_next).write(props13_ptr as *mut c_void);
    addr_of_mut!((*props13_ptr).p_next).write(ptr::null_mut::<c_void>());

    instance.get_physical_device_properties2(physical_device, props10_ptr.as_mut().unwrap());
    PhysicalDeviceProperties {
      p10: props10.assume_init().properties,
      p11: props11.assume_init(),
      p12: props12.assume_init(),
      p13: props13.assume_init(),
    }
  }
}

#[allow(unused)]
struct PhysicalDeviceFeatures<'a> {
  pub f10: vk::PhysicalDeviceFeatures,
  pub f11: vk::PhysicalDeviceVulkan11Features<'a>,
  pub f12: vk::PhysicalDeviceVulkan12Features<'a>,
  pub f13: vk::PhysicalDeviceVulkan13Features<'a>,
}

fn get_extended_features(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
) -> PhysicalDeviceFeatures {
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
    PhysicalDeviceFeatures {
      f10: features10.assume_init().features,
      f11: features11.assume_init(),
      f12: features12.assume_init(),
      f13: features13.assume_init(),
    }
  }
}
