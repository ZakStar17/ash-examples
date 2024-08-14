mod device_selector;
mod logical_device;
mod physical_device;
mod queues;
mod vendor;

use device_selector::select_physical_device;
pub use logical_device::Device;
pub use physical_device::PhysicalDevice;
pub use queues::{QueueFamilies, Queues};

use std::{
  ffi::{c_void, CStr},
  mem::MaybeUninit,
  ptr::{self, addr_of_mut},
};

use ash::vk;

use crate::utility::{self};

static MEMORY_PRIORITY: &CStr = c"VK_EXT_memory_priority";
static PAGEABLE_DEVICE_LOCAL_MEMORY: &CStr = c"VK_EXT_pageable_device_local_memory";

#[derive(Debug, Default)]
pub struct EnabledDeviceExtensions {
  memory_priority: bool,
  pageable_device_local_memory: bool,
  count: usize,
}

impl EnabledDeviceExtensions {
  pub fn mark_supported_by_physical_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
  ) -> Result<Self, vk::Result> {
    let properties = unsafe { instance.enumerate_device_extension_properties(physical_device)? };

    let mut supported = Self::default();

    // a bit inefficient as it retests for valid cstrings but at least doesn't do any allocations
    let is_supported = |ext| {
      properties
        .iter()
        .any(|props| unsafe { utility::i8_array_as_cstr(&props.extension_name) }.unwrap() == ext)
    };

    let mut supported_count = 0;
    if is_supported(MEMORY_PRIORITY) {
      supported.memory_priority = true;
      supported_count += 1;
    }
    if is_supported(PAGEABLE_DEVICE_LOCAL_MEMORY) {
      supported.pageable_device_local_memory = true;
      supported_count += 1;
    }

    supported.count = supported_count;
    Ok(supported)
  }

  pub fn get_extension_list(&self) -> Vec<*const i8> {
    let mut ptrs = Vec::with_capacity(self.count);
    if self.memory_priority {
      ptrs.push(MEMORY_PRIORITY.as_ptr());
    }
    if self.pageable_device_local_memory {
      ptrs.push(PAGEABLE_DEVICE_LOCAL_MEMORY.as_ptr());
    }

    ptrs
  }
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
