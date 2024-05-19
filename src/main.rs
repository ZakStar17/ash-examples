#![feature(vec_into_raw_parts)]

mod allocator;
mod command_pools;
mod create_objs;
mod descriptor_sets;
mod device_destroyable;
mod errors;
mod initialization;
mod pipelines;
mod renderer;
mod shaders;
mod utility;

use ash::vk;
use std::ffi::CStr;

use crate::renderer::Renderer;

// array of validation layers that should be loaded
// validation layers names should be valid cstrings (not contain null bytes nor invalid characters)
#[cfg(feature = "vl")]
const VALIDATION_LAYERS: [&CStr; 1] = [c"VK_LAYER_KHRONOS_validation"];
#[cfg(feature = "vl")]
pub const ADDITIONAL_VALIDATION_FEATURES: [vk::ValidationFeatureEnableEXT; 2] = [
  vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
  vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
];

const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

static APPLICATION_NAME: &CStr = c"Mandelbrot";
const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

static REQUIRED_DEVICE_EXTENSIONS: [&CStr; 0] = [];

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

fn initialize_and_run() -> Result<(), String> {
  let mut renderer = Renderer::initialize(IMAGE_WIDTH, IMAGE_HEIGHT, IMAGE_MINIMAL_SIZE)
    .map_err(|err| format!("Failed to initialize: {}", err))?;
  unsafe { renderer.record_work() }.map_err(|err| format!("Failed to record work: {}", err))?;

  println!("Submitting work...");
  renderer
    .submit_and_wait()
    .map_err(|err| format!("Failed to submit work: {}", err))?;
  println!("GPU finished!");

  println!("Saving file...");
  let mut save_result = Ok(());
  unsafe {
    renderer.get_resulting_data(|data| {
      save_result = image::save_buffer(
        IMAGE_SAVE_PATH,
        data,
        IMAGE_WIDTH,
        IMAGE_HEIGHT,
        IMAGE_SAVE_TYPE,
      );
    })
  }
  .map_err(|err| format!("Failed to get resulting data: {}", err))?;
  if let Err(err) = save_result {
    return Err(format!("Failed to save image: {}", err));
  }

  Ok(())
}

fn main() {
  env_logger::init();

  if let Err(s) = initialize_and_run() {
    log::error!("{}", s);
    std::process::exit(1);
  }

  println!("Done!");
}
