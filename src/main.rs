mod device;
mod entry;
mod errors;
mod instance;
mod utility;

// validation layers module will only exist if validation layers are enabled
#[cfg(feature = "vl")]
mod validation_layers;

use ash::vk;
use device::{DeviceCreationError, DeviceSelectionError};
use instance::InstanceCreationError;
use std::ffi::CStr;
use validation_layers::DebugUtilsMarker;

use crate::device::{Device, PhysicalDevice};

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

#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
  #[error("Failed to create a instance\n    {0}")]
  InstanceCreationFailed(#[from] InstanceCreationError),
  #[error("An error occurred during device selection:\n    {0}")]
  DeviceSelectionError(#[from] DeviceSelectionError),
  #[error("No suitable physical devices found")]
  NoSuitableDevices,
  #[error("Failed to create a logical device:\n    {0}")]
  DeviceCreationFailed(#[from] DeviceCreationError),
}

fn run_app() -> Result<(), ApplicationError> {
  // initialize env_logger with debug if validation layers are enabled, warn otherwise
  #[cfg(feature = "vl")]
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
  #[cfg(not(feature = "vl"))]
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

  let entry: ash::Entry = unsafe { entry::get_entry() };

  #[cfg(feature = "vl")]
  let (instance, mut debug_utils) = instance::create_instance(&entry)?;
  #[cfg(not(feature = "vl"))]
  let instance = instance::create_instance(&entry)?;

  let physical_device = match unsafe { PhysicalDevice::select(&instance)? } {
    Some(device) => device,
    None => {
      return Err(ApplicationError::NoSuitableDevices);
    }
  };

  let (logical_device, queues) = Device::create(&instance, &physical_device)?;

  #[cfg(feature = "vl")]
  let debug_marker = DebugUtilsMarker::new(&instance, &logical_device);
  #[cfg(feature = "vl")]
  unsafe {
    debug_marker.set_queue_labels(queues);
  }

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

  Ok(())
}

fn main() {
  if let Err(err) = run_app() {
    eprintln!("Instance creation failed:\n    {}", err);
    std::process::exit(1);
  }
}
