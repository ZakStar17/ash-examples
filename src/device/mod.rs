mod physical_device;
mod queues;
mod vendor;
mod logical_device;

use std::{
  ffi::c_void,
  mem::MaybeUninit,
  ptr::{self, addr_of_mut},
};

use ash::vk;
pub use physical_device::PhysicalDevice;
pub use logical_device::create_logical_device;
pub use queues::{QueueFamilies, Queues};

use crate::{
  device::vendor::Vendor,
  utility::{self, c_char_array_to_string},
  IMAGE_FORMAT, IMAGE_HEIGHT, IMAGE_WIDTH, REQUIRED_DEVICE_EXTENSIONS, TARGET_API_VERSION,
};

macro_rules! const_flag_bitor {
    ($t:ty, $x:expr, $($y:expr),+) => {
      // ash flags don't implement const bitor
      <$t>::from_raw(
        $x.as_raw() $(| $y.as_raw())+,
      )
    };
  }

// kinda overkill
const REQUIRED_FORMAT_IMAGE_FLAGS_OPTIMAL: vk::FormatFeatureFlags = const_flag_bitor!(
  vk::FormatFeatureFlags,
  vk::FormatFeatureFlags::TRANSFER_SRC,
  vk::FormatFeatureFlags::TRANSFER_DST
);
const REQUIRED_FORMAT_IMAGE_FLAGS_LINEAR: vk::FormatFeatureFlags =
  vk::FormatFeatureFlags::TRANSFER_DST;

const REQUIRED_IMAGE_USAGE_FLAGS_OPTIMAL: vk::ImageUsageFlags = const_flag_bitor!(
  vk::ImageUsageFlags,
  vk::ImageUsageFlags::TRANSFER_SRC,
  vk::ImageUsageFlags::TRANSFER_DST
);
const REQUIRED_IMAGE_USAGE_FLAGS_LINEAR: vk::ImageUsageFlags = vk::ImageUsageFlags::TRANSFER_DST;

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

fn check_format_support(instance: &ash::Instance, physical_device: vk::PhysicalDevice) -> bool {
  let properties =
    unsafe { instance.get_physical_device_format_properties(physical_device, IMAGE_FORMAT) };

  if !properties
    .optimal_tiling_features
    .contains(REQUIRED_FORMAT_IMAGE_FLAGS_OPTIMAL)
  {
    return false;
  }

  if !properties
    .linear_tiling_features
    .contains(REQUIRED_FORMAT_IMAGE_FLAGS_LINEAR)
  {
    return false;
  }

  true
}

fn check_image_size_support(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
  tiling: vk::ImageTiling,
  usage: vk::ImageUsageFlags,
) -> bool {
  let properties = unsafe {
    instance
      .get_physical_device_image_format_properties(
        physical_device,
        IMAGE_FORMAT,
        vk::ImageType::TYPE_2D,
        tiling,
        usage,
        vk::ImageCreateFlags::empty(),
      )
      .expect("Failed to query for image format properties")
  };
  log::debug!(
    "{} image {:?} properties: {:#?}",
    match tiling {
      vk::ImageTiling::LINEAR => "Linear",
      vk::ImageTiling::OPTIMAL => "Optimal",
      _ => panic!(),
    },
    IMAGE_FORMAT,
    properties
  );

  IMAGE_WIDTH <= properties.max_extent.width && IMAGE_HEIGHT <= properties.max_extent.height
}

fn check_linear_tiling_image_size_support(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
) -> bool {
  check_image_size_support(
    instance,
    physical_device,
    vk::ImageTiling::LINEAR,
    REQUIRED_IMAGE_USAGE_FLAGS_LINEAR,
  )
}

fn check_optimal_tiling_image_size_support(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
) -> bool {
  check_image_size_support(
    instance,
    physical_device,
    vk::ImageTiling::OPTIMAL,
    REQUIRED_IMAGE_USAGE_FLAGS_OPTIMAL,
  )
}

unsafe fn select_physical_device(
  instance: &ash::Instance,
) -> Option<(vk::PhysicalDevice, QueueFamilies)> {
  instance
    .enumerate_physical_devices()
    .expect("Failed to enumerate physical devices")
    .into_iter()
    .filter(|&physical_device| {
      // Filter devices that are not supported
      // You should check for any feature or limit support that your application might need

      let properties = instance.get_physical_device_properties(physical_device);
      log_device_properties(&properties);

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

      if !check_format_support(instance, physical_device) {
        log::warn!("Skipped physical device: Device does not support required formats");
        return false;
      }

      if !check_linear_tiling_image_size_support(instance, physical_device) || !check_optimal_tiling_image_size_support(instance, physical_device) {
        log::warn!("Skipped physical device: Application image size requirements are bigger than supported by the device");
        return false;
      }

      true
    })
    .filter_map(|physical_device| {
      match QueueFamilies::get_from_physical_device(instance, physical_device) {
        Err(()) => {
          log::info!("Skipped physical device: Device does not contain required queue families");
          None
        },
        Ok(families) => Some((physical_device, families))
      }
    })
    .min_by_key(|(physical_device, families)| {
      // Assign a score to each device and select the best one available
      // A full application may use multiple metrics like limits, queue families and even the
      //    device id to rank each device that a user can have

      let queue_family_importance = 3;
      let device_score_importance = 0;

      // rank devices by number of specialized queue families
      let queue_score = QueueFamilies::FAMILY_COUNT - families.unique_indices.len();

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
  let mut main_props: MaybeUninit<vk::PhysicalDeviceProperties2> = MaybeUninit::uninit();
  let mut props11: MaybeUninit<vk::PhysicalDeviceVulkan11Properties> = MaybeUninit::uninit();
  let main_props_ptr = main_props.as_mut_ptr();
  let props11_ptr = props11.as_mut_ptr();

  unsafe {
    addr_of_mut!((*props11_ptr).s_type)
      .write(vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_1_PROPERTIES);
    addr_of_mut!((*props11_ptr).p_next).write(ptr::null_mut::<c_void>());

    addr_of_mut!((*main_props_ptr).s_type).write(vk::StructureType::PHYSICAL_DEVICE_PROPERTIES_2);
    // requesting for Vulkan11Properties
    addr_of_mut!((*main_props_ptr).p_next).write(props11_ptr as *mut c_void);

    instance.get_physical_device_properties2(physical_device, main_props_ptr.as_mut().unwrap());

    (main_props.assume_init().properties, props11.assume_init())
  }
}
