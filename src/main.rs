#![feature(vec_into_raw_parts)]

mod allocator;
mod command_pools;
mod create_objs;
mod device_destroyable;
mod errors;
mod gpu_data;
mod initialization;
mod pipelines;
mod render_pass;
mod renderer;
mod shaders;
mod utility;
mod vertices;

use ash::vk;
use std::ffi::CStr;
use vertices::Vertex;

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

const IMAGE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
const IMAGE_FORMAT_SIZE: u64 = 4;
const IMAGE_MINIMAL_SIZE: u64 = IMAGE_WIDTH as u64 * IMAGE_HEIGHT as u64 * IMAGE_FORMAT_SIZE;

const IMAGE_SAVE_TYPE: image::ColorType = image::ColorType::Rgba8; // should be equivalent

const IMAGE_SAVE_PATH: &str = "image.png";

const BACKGROUND_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [0.01, 0.01, 0.01, 1.0],
};
const VERTICES: [Vertex; 3] = [
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
const INDICES: [u16; 3] = [0, 1, 2];

fn main() {
  env_logger::init();

  let mut renderer = Renderer::initialize(IMAGE_WIDTH, IMAGE_HEIGHT, IMAGE_MINIMAL_SIZE)
    .expect("Failed to initialize");
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
