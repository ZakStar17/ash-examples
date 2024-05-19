mod device;
mod entry;
mod errors;
mod instance;
mod utility;

// validation layers module will only exist if validation layers are enabled
#[cfg(feature = "vl")]
mod validation_layers;

use ash::vk;
use std::ffi::CStr;

use crate::device::{create_logical_device, PhysicalDevice};

// validation layers names should be valid cstrings (not contain null bytes nor invalid characters)
#[cfg(feature = "vl")]
const VALIDATION_LAYERS: [&CStr; 1] = [c"VK_LAYER_KHRONOS_validation"];
#[cfg(feature = "vl")]
const ADDITIONAL_VALIDATION_FEATURES: [vk::ValidationFeatureEnableEXT; 2] = [
  vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
  vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
];

// Vulkan API version required to run the program
// You may have to use an older API version if you want to support devices that do not yet support
// the recent versions. You can see in the documentation what is the minimum supported version
// for each extension, feature or API call.
const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

// somewhat arbitrary
static APPLICATION_NAME: &CStr = c"Vulkan Device Creation";
const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

static REQUIRED_DEVICE_EXTENSIONS: [&CStr; 0] = [];

fn main() {
  env_logger::init();

  let entry: ash::Entry = unsafe { entry::get_entry() };

  let on_instance_fail = |err| {
    log::error!("Failed to create an instance: {}", err);
    std::process::exit(1);
  };
  #[cfg(feature = "vl")]
  let (instance, mut debug_utils) = match instance::create_instance(&entry) {
    Ok(v) => v,
    Err(err) => on_instance_fail(err),
  };
  #[cfg(not(feature = "vl"))]
  let instance = match instance::create_instance(&entry) {
    Ok(v) => v,
    Err(err) => on_instance_fail(err),
  };

  let physical_device = match unsafe { PhysicalDevice::select(&instance) } {
    Ok(device_opt) => match device_opt {
      Some(device) => device,
      None => {
        log::error!("No suitable device found");
        std::process::exit(1);
      }
    },
    Err(err) => {
      log::error!("Failed to query physical devices: {:?}", err);
      std::process::exit(1);
    }
  };

  let (logical_device, _queues) = match create_logical_device(&instance, &physical_device) {
    Ok(v) => v,
    Err(err) => {
      log::error!("Failed to create an logical device: {}", err);
      std::process::exit(1);
    }
  };

  println!("Successfully created the logical device!");

  log::debug!("Destroying objects");
  unsafe {
    // wait until all operations have finished and the device is safe to destroy
    logical_device
      .device_wait_idle()
      .expect("Failed to wait for the device to become idle");

    // destroying a logical device also implicitly destroys all associated queues
    logical_device.destroy_device(None);

    #[cfg(feature = "vl")]
    debug_utils.destroy_self();
    instance.destroy_instance(None);
  }
}
