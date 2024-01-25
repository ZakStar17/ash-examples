use ash::vk;
use log::{debug, info};

use crate::{
  utility::{self, c_char_array_to_string},
  REQUIRED_DEVICE_EXTENSIONS, TARGET_API_VERSION,
};

#[derive(Debug)]
pub struct QueueFamily {
  pub index: u32,
  pub queue_count: u32,
}

// Specialized compute and transfer queue families may not be available
// If so, they will be substituted by the graphics queue family, as a queue family that supports
//    graphics implicitly also supports compute and transfer operations
#[derive(Debug)]
pub struct QueueFamilies {
  pub graphics: QueueFamily,
  pub compute: Option<QueueFamily>,
  pub transfer: Option<QueueFamily>,
  pub unique_indices: Box<[u32]>,
}

enum Vendor {
  NVIDIA,
  AMD,
  ARM,
  INTEL,
  ImgTec,
  Qualcomm,
  Unknown(u32),
}

// support struct for displaying vendor information
impl Vendor {
  fn from_id(id: u32) -> Self {
    // some known ids
    match id {
      0x1002 => Self::AMD,
      0x1010 => Self::ImgTec,
      0x10DE => Self::NVIDIA,
      0x13B5 => Self::ARM,
      0x5143 => Self::Qualcomm,
      0x8086 => Self::INTEL,
      _ => Self::Unknown(id),
    }
  }

  fn parse_driver_version(&self, v: u32) -> String {
    // Different vendors can use their own version formats
    // The Vulkan format is (3 bits), major (7 bits), minor (10 bits), patch (12 bits), so vendors
    // with other formats need their own parsing code
    match self {
      Self::NVIDIA => {
        // major (10 bits), minor (8 bits), secondary branch (8 bits), tertiary branch (6 bits)
        let eight_bits = 0b11111111;
        let six_bits = 0b111111;
        format!(
          "{}.{}.{}.{}",
          v >> (32 - 10),
          v >> (32 - 10 - 8) & eight_bits,
          v >> (32 - 10 - 8 - 8) & eight_bits,
          v & six_bits
        )
      }
      _ => utility::parse_vulkan_api_version(v),
    }
  }
}

impl ToString for Vendor {
  fn to_string(&self) -> String {
    match self {
      Self::NVIDIA => "NVIDIA".to_owned(),
      Self::AMD => "AMD".to_owned(),
      Self::ARM => "ARM".to_owned(),
      Self::INTEL => "INTEL".to_owned(),
      Self::ImgTec => "ImgTec".to_owned(),
      Self::Qualcomm => "Qualcomm".to_owned(),
      Self::Unknown(id) => format!("Unknown ({})", id),
    }
  }
}

fn log_device_properties(properties: &vk::PhysicalDeviceProperties) {
  let vendor = Vendor::from_id(properties.vendor_id);
  let driver_version = vendor.parse_driver_version(properties.driver_version);

  info!(
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

fn check_extension_support(instance: &ash::Instance, device: &vk::PhysicalDevice) -> bool {
  let properties = unsafe {
    instance
      .enumerate_device_extension_properties(*device)
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

pub unsafe fn select_physical_device(
  instance: &ash::Instance,
) -> (vk::PhysicalDevice, QueueFamilies) {
  let (physical_device, queue_families) = instance
    .enumerate_physical_devices()
    .expect("Failed to enumerate physical devices")
    .into_iter()
    .filter(|physical_device| {
      // Filter devices that are not supported
      // You should check for any feature or limit support that your application might need

      let properties = instance.get_physical_device_properties(*physical_device);
      log_device_properties(&properties);

      if properties.api_version < TARGET_API_VERSION {
        info!(
          "Skipped physical device: Device API version is less than targeted by the application"
        );
        return false;
      }

      // check if device supports all required extensions
      if !check_extension_support(instance, physical_device) {
        info!("Skipped physical device: Device does not support all required extensions");
        return false;
      }

      true
    })
    .filter_map(|physical_device| {
      // Filter devices that not support specific queue families
      // Your application may not need any graphics capabilities or otherwise need features only
      //    supported by specific queues, so alter to your case accordingly
      // Generally you only need one queue from each family unless you are doing highly concurrent
      //    operations

      let mut graphics = None;
      let mut compute = None;
      let mut transfer = None;
      for (i, family) in instance
        .get_physical_device_queue_family_properties(physical_device)
        .iter()
        .enumerate()
      {
        if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
          graphics = Some(QueueFamily {
            index: i as u32,
            queue_count: family.queue_count,
          });
        } else if family.queue_flags.contains(vk::QueueFlags::COMPUTE) {
          // only set if family does not contain graphics flag
          compute = Some(QueueFamily {
            index: i as u32,
            queue_count: family.queue_count,
          });
        } else if family.queue_flags.contains(vk::QueueFlags::TRANSFER) {
          // only set if family does not contain graphics nor compute flag
          transfer = Some(QueueFamily {
            index: i as u32,
            queue_count: family.queue_count,
          });
        }
      }

      if graphics.is_none() {
        info!("Skipped physical device: Device does not support graphics");
        return None;
      }

      // commonly used
      let unique_indices = [graphics.as_ref(), compute.as_ref(), transfer.as_ref()]
        .into_iter()
        .filter_map(|opt| opt.map(|f| f.index))
        .collect();

      Some((
        physical_device,
        QueueFamilies {
          graphics: graphics.unwrap(),
          compute,
          transfer,
          unique_indices,
        },
      ))
    })
    .min_by_key(|(physical_device, families)| {
      // Assign a score to each device and select the best one available
      // A full application may use multiple metrics like limits, queue families and even the
      //    device id to rank each device that a user can have

      let queue_family_importance = 3;
      let device_score_importance = 0;

      // rank devices by number of specialized queue families
      let queue_score = 3 - families.unique_indices.len();

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
    .expect("No supported physical device available");

  let selected_properties = instance.get_physical_device_properties(physical_device);
  info!(
    "Using physical device \"{}\"",
    c_char_array_to_string(&selected_properties.device_name)
  );

  print_debug_info(instance, physical_device);

  (physical_device, queue_families)
}

fn print_debug_info(instance: &ash::Instance, physical_device: vk::PhysicalDevice) {
  let mem_properties = unsafe { instance.get_physical_device_memory_properties(physical_device) };
  debug!("Available memory heaps:");
  for i in 0..mem_properties.memory_heap_count {
    let heap = mem_properties.memory_heaps[i as usize];
    let heap_flags = if heap.flags.is_empty() {
      String::from("no heap flags")
    } else {
      format!("heap flags {:?}", heap.flags)
    };
    let mem_type_flags: Vec<vk::MemoryPropertyFlags> = mem_properties.memory_types
      [0..(mem_properties.memory_type_count as usize)]
      .iter()
      .filter_map(|mem_type| {
        if mem_type.heap_index == i {
          Some(mem_type.property_flags)
        } else {
          None
        }
      })
      .collect();
    debug!(
      "  {} -> {}mb with {} and {:?} memory type flags",
      i,
      heap.size / 1000000,
      heap_flags,
      mem_type_flags
    );
  }
}
