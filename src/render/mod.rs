mod engine;
mod frame;
mod objects;
mod render_object;
mod renderer;
mod shaders;
mod sync_renderer;
mod texture;
mod vertex;

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

pub const FRAMES_IN_FLIGHT: usize = 2;

pub const BACKGROUND_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [0.01, 0.01, 0.01, 1.0],
};

pub use engine::RenderEngine;
pub use render_object::RenderPosition;
