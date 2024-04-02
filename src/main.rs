mod allocator;
mod command_pools;
mod create_objs;
mod descriptor_sets;
mod device;
mod device_destroyable;
mod entry;
mod errors;
mod instance;
mod pipeline;
mod pipeline_cache;
mod renderer;
mod shaders;
mod utility;

// validation layers module will only exist if validation layers are enabled
#[cfg(feature = "vl")]
mod validation_layers;

use ash::vk;
use std::ffi::CStr;

use crate::renderer::Renderer;

// array of validation layers that should be loaded
// validation layers names should be valid cstrings (not contain null bytes nor invalid characters)
#[cfg(feature = "vl")]
pub const VALIDATION_LAYERS: [&'static CStr; 1] = [cstr!("VK_LAYER_KHRONOS_validation")];
#[cfg(feature = "vl")]
pub const ADDITIONAL_VALIDATION_FEATURES: [vk::ValidationFeatureEnableEXT; 2] = [
  vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
  vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
];

pub const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

// somewhat arbitrary
pub const APPLICATION_NAME: &'static CStr = cstr!("Mandelbrot");
pub const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

pub const REQUIRED_DEVICE_EXTENSIONS: [&'static CStr; 0] = [];

const IMAGE_WIDTH: u32 = 4000;
const IMAGE_HEIGHT: u32 = 4000;

const IMAGE_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
const IMAGE_SAVE_TYPE: ::image::ColorType = ::image::ColorType::Rgba8; // should be equivalent
const IMAGE_FORMAT_SIZE: u64 = 4;
const IMAGE_MINIMAL_SIZE: u64 = IMAGE_WIDTH as u64 * IMAGE_HEIGHT as u64 * IMAGE_FORMAT_SIZE;

const IMAGE_SAVE_PATH: &str = "./image.png";

// Size of each local group in the shader invocation
// Normally these would be calculated from image dimensions and clapped to respect device limits
// but for this example a small local group size should suffice (limits are still checked in
// physical device selection)
const SHADER_GROUP_SIZE_X: u32 = 16;
const SHADER_GROUP_SIZE_Y: u32 = 16;

// mandelbrot constants
// these are passed as specialization constants in the shader
const MAX_ITERATIONS: u32 = 10000;
const FOCAL_POINT: [f32; 2] = [-0.765, 0.0]; // complex plane coordinates of the image center
const ZOOM: f32 = 0.40486;

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
