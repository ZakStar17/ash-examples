mod frame;
mod objects;
mod render;
mod renderer;
mod shaders;
mod sync_renderer;
mod vertex;

use std::ffi::CStr;

use ash::vk;

use self::vertex::Vertex;
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

pub const VERTICES: [Vertex; 3] = [
  Vertex {
    pos: [0.7, 0.3],
    color: [1.0, 0.0, 0.0],
  },
  Vertex {
    pos: [-0.4, 0.9],
    color: [0.0, 1.0, 0.0],
  },
  Vertex {
    pos: [-0.9, -0.8],
    color: [0.0, 0.0, 1.0],
  },
];
pub const INDICES: [u16; 3] = [0, 1, 2];

pub const BACKGROUND_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [0.01, 0.01, 0.01, 1.0],
};

pub use render::Render;
