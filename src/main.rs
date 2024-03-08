mod allocator;
mod command_pools;
mod device;
mod entry;
mod errors;
mod image;
mod instance;
mod renderer;
mod utility;

// validation layers module will only exist if validation layers are enabled
#[cfg(feature = "vl")]
mod validation_layers;

use ash::vk;
use std::ffi::CStr;

use crate::renderer::Renderer;

// validation layers names should be valid cstrings (not contain null bytes nor invalid characters)
#[cfg(feature = "vl")]
pub const VALIDATION_LAYERS: [&'static CStr; 1] = [cstr!("VK_LAYER_KHRONOS_validation")];
#[cfg(feature = "vl")]
pub const ADDITIONAL_VALIDATION_FEATURES: [vk::ValidationFeatureEnableEXT; 2] = [
  vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
  vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
];

pub const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

pub const APPLICATION_NAME: &'static CStr = cstr!("Image clear");
pub const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

pub const REQUIRED_DEVICE_EXTENSIONS: [&'static CStr; 0] = [];

pub const IMAGE_WIDTH: u32 = 1920;
pub const IMAGE_HEIGHT: u32 = 1080;

// device selection checks if format is available
pub const IMAGE_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
pub const IMAGE_SAVE_TYPE: ::image::ColorType = ::image::ColorType::Rgba8; // should be equivalent
                                                                           // valid color values depend on IMAGE_FORMAT
pub const IMAGE_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [134.0 / 255.0, 206.0 / 255.0, 203.0 / 255.0, 1.0], // rgba(134, 206, 203, 255)
};

const IMAGE_SAVE_PATH: &str = "image.png";

fn main() {
  env_logger::init();

  let mut renderer = Renderer::initialize().expect("Failed to initialize");
  unsafe { renderer.record_work() }.expect("Failed to record work");

  println!("Submitting work...");
  renderer.submit_and_wait();
  println!("GPU finished!");

  println!("Saving file...");
  renderer.save_buffer_to_image_file(IMAGE_SAVE_PATH);
  println!("Done!");
}
