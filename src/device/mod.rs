mod logical_device;
mod physical_device;
mod queues;
mod vendor;

use std::{
  ffi::c_void,
  mem::MaybeUninit,
  ptr::{self, addr_of_mut},
};

use ash::vk;
pub use logical_device::create_logical_device;
pub use physical_device::PhysicalDevice;
pub use queues::{QueueFamilies, Queues};

use crate::{
  const_flag_bitor, device::vendor::Vendor, utility::{self, c_char_array_to_string, i8_array_as_cstr}, IMAGE_FORMAT, IMAGE_HEIGHT, IMAGE_MINIMAL_SIZE, IMAGE_WIDTH, REQUIRED_DEVICE_EXTENSIONS, TARGET_API_VERSION
};

const REQUIRED_IMAGE_FORMAT_FEATURES: vk::FormatFeatureFlags = const_flag_bitor!(
  vk::FormatFeatureFlags,
  vk::FormatFeatureFlags::TRANSFER_SRC,
  vk::FormatFeatureFlags::TRANSFER_DST
);

const REQUIRED_IMAGE_USAGES: vk::ImageUsageFlags = const_flag_bitor!(
  vk::ImageUsageFlags,
  vk::ImageUsageFlags::TRANSFER_SRC,
  vk::ImageUsageFlags::TRANSFER_DST
);

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

fn supports_required_extensions(
  instance: &ash::Instance,
  device: vk::PhysicalDevice,
) -> Result<bool, vk::Result> {
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

fn supports_required_image_formats(instance: &ash::Instance, physical_device: vk::PhysicalDevice) -> bool {
  let properties =
    unsafe { instance.get_physical_device_format_properties(physical_device, IMAGE_FORMAT) };

  if !properties
    .optimal_tiling_features
    .contains(REQUIRED_IMAGE_FORMAT_FEATURES)
  {
    return false;
  }

  true
}

fn supports_image_dimensions(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
  tiling: vk::ImageTiling,
  usage: vk::ImageUsageFlags,
) -> Result<bool, vk::Result> {
  let properties = unsafe {
    instance
      .get_physical_device_image_format_properties(
        physical_device,
        IMAGE_FORMAT,
        vk::ImageType::TYPE_2D,
        tiling,
        usage,
        vk::ImageCreateFlags::empty(),
      )?
  };
  log::debug!(
    "image {:?} properties: {:#?}",
    IMAGE_FORMAT,
    properties
  );


  Ok(IMAGE_WIDTH <= properties.max_extent.width && IMAGE_HEIGHT <= properties.max_extent.height && IMAGE_MINIMAL_SIZE <= properties.max_resource_size)
}

unsafe fn select_physical_device(
  instance: &ash::Instance,
) -> Result<
  Option<(
    vk::PhysicalDevice,
    PhysicalDeviceProperties,
    PhysicalDeviceFeatures,
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

        if properties.p10.api_version < TARGET_API_VERSION {
          log::info!(
            "Skipped physical device: Device API version is less than targeted by the application"
          );
          return None;
        }

        match supports_required_extensions(instance, physical_device) {
          Ok(supports_extensions) => {
            if !supports_extensions {
              log::info!(
                "Skipped physical device: Device does not support all required extensions"
              );
              return None;
            }
          }
          Err(err) => {
            log::error!("Device selection error: {:?}", err);
            return None;
          }
        }

        if !supports_required_image_formats(instance, physical_device) {
          log::warn!("Skipped physical device: Device does not support all required image formats");
          return None;
        }

        match supports_image_dimensions(instance, physical_device, vk::ImageTiling::OPTIMAL, REQUIRED_IMAGE_USAGES) {
          Ok(supports_dimensions) => {
            if !supports_dimensions {
              log::error!(
                "Skipped physical device: Device does not required image dimensions"
              );
              return None;
            }
          }
          Err(err) => {
            log::error!("Device selection error: {:?}", err);
            return None;
          }
        }


        if features.f12.timeline_semaphore != vk::TRUE {
          log::warn!("Skipped physical device: Device does not support timeline semaphores");
          return None;
        }

        if features.f13.synchronization2 != vk::TRUE {
          log::warn!("Skipped physical device: Device does not support synchronization features");
          return None;
        }

        Some((physical_device, properties, features))
      })
      .filter_map(|(physical_device, properties, features)| {
        // filter devices that do not have required queue families
        match QueueFamilies::get_from_physical_device(instance, physical_device) {
          Err(()) => {
            log::info!("Skipped physical device: Device does not contain required queue families");
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
pub struct PhysicalDeviceProperties {
  pub p10: vk::PhysicalDeviceProperties,
  pub p11: vk::PhysicalDeviceVulkan11Properties,
  pub p12: vk::PhysicalDeviceVulkan12Properties,
  pub p13: vk::PhysicalDeviceVulkan13Properties,
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
pub struct PhysicalDeviceFeatures {
  pub f10: vk::PhysicalDeviceFeatures,
  pub f11: vk::PhysicalDeviceVulkan11Features,
  pub f12: vk::PhysicalDeviceVulkan12Features,
  pub f13: vk::PhysicalDeviceVulkan13Features,
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
