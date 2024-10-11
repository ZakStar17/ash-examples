use ash::vk::{self};
use std::{
  marker::PhantomData,
  ops::Deref,
  os::raw::c_void,
  ptr::{self},
};

use super::{EnabledDeviceExtensions, PhysicalDevice, SingleQueues};

pub struct Device {
  pub inner: ash::Device,
  pub enabled_extensions: EnabledDeviceExtensions,
}

impl Deref for Device {
  type Target = ash::Device;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl Device {
  pub fn create(
    instance: &ash::Instance,
    physical_device: &PhysicalDevice,
  ) -> Result<(Self, SingleQueues), vk::Result> {
    let (queue_create_infos, unique_queue_size) =
      super::queues::get_single_queue_create_infos(&physical_device.queue_families);

    let to_enable_extensions =
      EnabledDeviceExtensions::mark_supported_by_physical_device(instance, **physical_device)?;
    let extension_ptrs = to_enable_extensions.get_extension_list();

    log::debug!(
      "Enabling the following device extensions:\n{:#?}",
      to_enable_extensions
    );

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
      queue_create_info_count: unique_queue_size as u32,
      p_enabled_features: &features10,
      p_next: &features12 as *const vk::PhysicalDeviceVulkan12Features as *const c_void,
      pp_enabled_layer_names: ptr::null(), // deprecated
      enabled_layer_count: 0,              // deprecated
      pp_enabled_extension_names: extension_ptrs.as_ptr(),
      enabled_extension_count: extension_ptrs.len() as u32,
      flags: vk::DeviceCreateFlags::empty(),
      _marker: PhantomData,
    };
    log::debug!("Creating logical device");
    let device: ash::Device =
      unsafe { instance.create_device(**physical_device, &create_info, None)? };

    let queues = unsafe {
      let queue_create_infos = &queue_create_infos[0..unique_queue_size];
      super::queues::retrieve_single_queues(
        &device,
        &physical_device.queue_families,
        queue_create_infos,
      )
    };
    log::debug!("Queue families:\n{:#?}", physical_device.queue_families);
    log::debug!("Queue addresses:\n{:#?}", queues);

    Ok((
      Self {
        inner: device,
        enabled_extensions: to_enable_extensions,
      },
      queues,
    ))
  }
}
