mod entry;
mod instance;
mod logical_device;
mod physical_device;
mod utility;

// validation layers module will only exist if validation layers are enabled
#[cfg(feature = "vl")]
mod validation_layers;

use ash::vk;
use std::ffi::CStr;

use crate::physical_device::select_physical_device;

// simple macro to transmute literals to static CStr
macro_rules! cstr {
  ( $s:literal ) => {{
    unsafe { std::mem::transmute::<_, &CStr>(concat!($s, "\0")) }
  }};
}

// array of validation layers that should be loaded
// validation layers names should be valid cstrings (not contain null bytes nor invalid characters)
#[cfg(feature = "vl")]
pub const VALIDATION_LAYERS: [&'static CStr; 1] = [cstr!("VK_LAYER_KHRONOS_validation")];
#[cfg(feature = "vl")]
pub const ADDITIONAL_VALIDATION_FEATURES: [vk::ValidationFeatureEnableEXT; 2] = [
  vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
  vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
];

// Vulkan API version required to run the program
// In your case you may request a optimal version of the API in order to use specific features
// but fallback to an older version if the target is not supported by the driver or any physical
// device
pub const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

// somewhat arbitrary
pub const APPLICATION_NAME: &'static CStr = cstr!("Vulkan Instance creation");
pub const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

pub const REQUIRED_DEVICE_EXTENSIONS: [&'static CStr; 0] = [];

fn main() {
  env_logger::init();

  let entry: ash::Entry = unsafe { entry::get_entry() };

  #[cfg(feature = "vl")]
  let (instance, mut debug_utils) = instance::create_instance(&entry);
  #[cfg(not(feature = "vl"))]
  let instance = instance::create_instance(&entry);

  let (physical_device, queue_family_indices) = unsafe { select_physical_device(&instance) };

  let (logical_device, _queues) =
    logical_device::create_logical_device(&instance, &physical_device, &queue_family_indices);

  println!("Successfully created the logical device!");

  // Cleanup
  unsafe {
    // wait until all operations have finished and the device is safe to destroy
    logical_device
      .device_wait_idle()
      .expect("Failed to wait for the device to become idle");

    // destroying a logical device also implicitly destroys all associated queues
    log::debug!("Destroying logical device");
    logical_device.destroy_device(None);

    #[cfg(feature = "vl")]
    {
      log::debug!("Destroying debug utils messenger");
      debug_utils.destroy_self();
    }

    log::debug!("Destroying Instance");
    instance.destroy_instance(None);
  }
}
