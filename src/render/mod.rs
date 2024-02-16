mod engine;
mod frame;
mod objects;
pub mod push_constants;
mod renderer;
mod shaders;
pub mod sprites;
mod sync_renderer;
mod vertices;

use std::ffi::CStr;

use ash::vk;

use crate::utility::cstr;

pub use engine::RenderEngine;
pub use vertices::Vertex;

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
  float32: [0.005, 0.005, 0.005, 1.0],
};
// color exterior the game area
// (that appears if window is resized to a size with ratio different that in RESOLUTION)
pub const OUT_OF_BOUNDS_AREA_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [0.0, 0.0, 0.0, 1.0],
};

const RENDER_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;

const TEXTURE_PATH: &'static str = "./sprites.png";

#[repr(C)]
#[derive(Debug)]
struct ComputeOutput {
  collision: u32,
}
