use ash::vk;
use log::{debug, info};

use crate::utility::{self, c_char_array_to_string};

#[derive(Debug)]
pub struct QueueFamily {
  pub index: u32,
  pub queue_count: u32,
}

#[derive(Debug)]
pub struct QueueFamilies {
  pub graphics: QueueFamily,
  pub compute: Option<QueueFamily>,
  pub transfer: Option<QueueFamily>,
  pub unique_indices: Vec<u32>,
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

impl Vendor {
  fn from_id(id: u32) -> Self {
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
    // different vendors can use their own version formats
    // add vendor specific parsing here if different from Vulkan, which is
    // variant (3 bits), major (7 bits), minor (10 bits), patch (12 bits)
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
    Api Version: {},
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

pub unsafe fn select_physical_device(
  instance: &ash::Instance,
  required_device_extensions: &[String],
) -> (vk::PhysicalDevice, QueueFamilies) {
  let (physical_device, queue_family) = instance
    .enumerate_physical_devices()
    .expect("Failed to enumerate physical devices")
    .into_iter()
    .filter(|physical_device| {
      // filter devices that are not supported

      let properties = instance.get_physical_device_properties(*physical_device);
      log_device_properties(&properties);

      if !check_extension_support(instance, physical_device, required_device_extensions) {
        info!("Skipped physical device: Device does not support required extensions");
        return false;
      }

      true
    })
    .filter_map(|physical_device| {
      // filter devices that not support specific queue families

      let mut graphics = None;
      let mut compute = None;
      let mut transfer = None;
      for (i, family) in instance
        .get_physical_device_queue_family_properties(physical_device)
        .iter()
        .enumerate()
      {
        let obj = QueueFamily {
          index: i as u32,
          queue_count: family.queue_count,
        };
        if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
          graphics = Some(obj);
        } else if family.queue_flags.contains(vk::QueueFlags::COMPUTE) {
          // only set if family does not contain graphics flag
          // if a dedicated compute family is not found, the application will use the graphics one
          compute = Some(obj);
        } else if family.queue_flags.contains(vk::QueueFlags::TRANSFER) {
          // only set if family does not contain graphics nor compute flag
          // if a dedicated transfer family is not found, the application will use the graphics one
          transfer = Some(obj);
        }
      }

      if graphics.is_none() {
        if !instance
          .get_physical_device_queue_family_properties(physical_device)
          .into_iter()
          .any(|family| family.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        {
          info!("Skipped physical device: Device does not support graphics");
          return None;
        }
      }

      let mut unique_indices = Vec::with_capacity(3);
      unique_indices.push(graphics.as_ref().unwrap().index);
      if let Some(f) = compute.as_ref() {
        unique_indices.push(f.index);
      }
      if let Some(f) = transfer.as_ref() {
        unique_indices.push(f.index);
      }

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
    .min_by_key(|(physical_device, _)| {
      // select the best device out of the available
      // a full application may use multiple metrics like limits, queue families and even the
      //    device id to rank each device that a user can have
      // here we will just rank the devices by type and select the commonly most powerful one
      let t = instance
        .get_physical_device_properties(*physical_device)
        .device_type;
      if t == vk::PhysicalDeviceType::DISCRETE_GPU {
        0
      } else if t == vk::PhysicalDeviceType::INTEGRATED_GPU {
        1
      } else if t == vk::PhysicalDeviceType::VIRTUAL_GPU {
        2
      } else if t == vk::PhysicalDeviceType::CPU {
        3
      } else if t == vk::PhysicalDeviceType::OTHER {
        4
      } else {
        5
      }
    })
    .expect("No supported physical device available");

  print_debug_info(instance, physical_device);

  (physical_device, queue_family)
}

fn print_debug_info(instance: &ash::Instance, physical_device: vk::PhysicalDevice) {
  let mem_properties = unsafe { instance.get_physical_device_memory_properties(physical_device) };
  debug!("Available memory heaps:");
  for i in 0..mem_properties.memory_heap_count {
    let heap = mem_properties.memory_heaps[i as usize];
    let flags = if heap.flags.is_empty() {
      String::from("no flags")
    } else {
      format!("flags {:?}", heap.flags)
    };
    debug!("{}: {}mb with {}", i, heap.size / 1000000, flags);
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
    debug!("Available type flags: {:?}", mem_type_flags)
  }
}

fn check_extension_support(
  instance: &ash::Instance,
  device: &vk::PhysicalDevice,
  extensions: &[String],
) -> bool {
  let available_extensions = unsafe {
    instance
      .enumerate_device_extension_properties(*device)
      .expect("Failed to get device extension properties.")
  };

  let mut available_extensions: Vec<String> = available_extensions
    .into_iter()
    .map(|prop| utility::c_char_array_to_string(&prop.extension_name))
    .collect();

  debug!("Available device extensions: {:?}", available_extensions);

  match utility::contains_all(&mut available_extensions, extensions) {
    Ok(_) => true,
    Err(_) => false,
  }
}
