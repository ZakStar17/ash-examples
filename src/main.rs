mod entry;
mod instance;
mod utility;

// validation layers module will only exist if validation layers are enabled
#[cfg(feature = "vl")]
mod validation_layers;

use ash::vk;
use std::ffi::CStr;

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
// Some features or API calls may have to be substituted with older ones if the device or 
// driver doesn't support them
pub const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

// somewhat arbitrary
pub const APPLICATION_NAME: &'static CStr = cstr!("Vulkan Instance Creation");
pub const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

fn main() {
  env_logger::init();

  let entry: ash::Entry = unsafe { entry::get_entry() };

  #[cfg(feature = "vl")]
  let (instance, mut debug_utils) = instance::create_instance(&entry);
  #[cfg(not(feature = "vl"))]
  let instance = instance::create_instance(&entry);

  println!("Successfully created an Instance!");

  log::debug!("Destroying objects");
  unsafe {
    #[cfg(feature = "vl")]
    {
      debug_utils.destroy_self();
    }
    instance.destroy_instance(None);
  }
}
