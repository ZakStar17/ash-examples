mod allocator;
mod command_pools;
mod device;
mod entry;
mod errors;
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
const VALIDATION_LAYERS: [&'static CStr; 1] = [cstr!("VK_LAYER_KHRONOS_validation")];
#[cfg(feature = "vl")]
const ADDITIONAL_VALIDATION_FEATURES: [vk::ValidationFeatureEnableEXT; 2] = [
  vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
  vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
];

const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

const APPLICATION_NAME: &'static CStr = cstr!("Image clear");
const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

const REQUIRED_DEVICE_EXTENSIONS: [&'static CStr; 0] = [];

const IMAGE_WIDTH: u32 = 1920;
const IMAGE_HEIGHT: u32 = 1080;

const IMAGE_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
const IMAGE_FORMAT_SIZE: u64 = 4;
const IMAGE_MINIMAL_SIZE: u64 = IMAGE_WIDTH as u64 * IMAGE_HEIGHT as u64 * IMAGE_FORMAT_SIZE;

const IMAGE_SAVE_TYPE: image::ColorType = image::ColorType::Rgba8; // should be equivalent
                                                                   // valid color values depend on IMAGE_FORMAT
const IMAGE_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [134.0 / 255.0, 206.0 / 255.0, 203.0 / 255.0, 1.0], // rgba(134, 206, 203, 255)
};

const IMAGE_SAVE_PATH: &str = "image.png";

fn main() {
  env_logger::init();

  let mut renderer = Renderer::initialize(IMAGE_WIDTH, IMAGE_HEIGHT, IMAGE_MINIMAL_SIZE).expect("Failed to initialize");
  unsafe { renderer.record_work() }.expect("Failed to record work");

  println!("Submitting work...");
  renderer.submit_and_wait().expect("Failed to submit work");
  println!("GPU finished!");

  println!("Saving file...");
  unsafe {
    renderer.get_resulting_data(|data| {
      image::save_buffer(
        IMAGE_SAVE_PATH,
        data,
        IMAGE_WIDTH,
        IMAGE_HEIGHT,
        IMAGE_SAVE_TYPE,
      )
      .expect("Failed to save image");
    })
  }
  .expect("Failed to get resulting data");
  println!("Done!");
}
