mod device;
mod objects;
mod render;
mod renderer;
mod sync_renderer;

use std::ffi::CStr;

use ash::vk;

use crate::utility::cstr;

const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

const REQUIRED_DEVICE_EXTENSIONS: [&'static CStr; 1] = [cstr!("VK_KHR_swapchain")];

#[cfg(feature = "vl")]
pub const VALIDATION_LAYERS: [&'static CStr; 1] =
  [crate::utility::cstr!("VK_LAYER_KHRONOS_validation")];
#[cfg(feature = "vl")]
pub const ADDITIONAL_VALIDATION_FEATURES: [vk::ValidationFeatureEnableEXT; 2] = [
  vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
  vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
];

pub use render::Render;
