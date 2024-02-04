mod command_pools;
mod constant_buffers;
mod device;
mod entry;
mod image;
mod instance;
mod pipeline;
mod pipeline_cache;
mod render_pass;
mod shaders;
mod utility;
mod vertex;

// validation layers module will only exist if validation layers are enabled
#[cfg(feature = "vl")]
mod validation_layers;

use ash::vk;
use command_pools::TransferCommandBufferPool;
use device::PhysicalDevice;
use image::Image;
use std::{
  ffi::CStr,
  ops::BitOr,
  ptr::{self, addr_of},
};
use utility::cstr;
use vertex::Vertex;

use crate::{
  command_pools::GraphicsCommandBufferPool,
  constant_buffers::ConstantBuffers,
  pipeline::GraphicsPipeline,
  render_pass::{create_framebuffer, create_render_pass},
};

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

pub const APPLICATION_NAME: &'static CStr = cstr!("Image clear");
pub const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

pub const REQUIRED_DEVICE_EXTENSIONS: [&'static CStr; 0] = [];

pub const IMAGE_WIDTH: u32 = 1920;
pub const IMAGE_HEIGHT: u32 = 1080;

// should be compatible with fragment shader output
pub const IMAGE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
pub const IMAGE_SAVE_TYPE: ::image::ColorType = ::image::ColorType::Rgba8; // should be equivalent
                                                                           // valid color values depend on IMAGE_FORMAT
const IMAGE_SAVE_PATH: &str = "image.png";

pub const BACKGROUND_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [0.01, 0.01, 0.01, 1.0],
};

pub const VERTEX_COUNT: usize = 3;
pub const VERTICES: [Vertex; VERTEX_COUNT] = [
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
pub const INDEX_COUNT: usize = 3;
pub const INDICES: [u16; 3] = [0, 1, 2];

fn create_semaphore(device: &ash::Device) -> vk::Semaphore {
  let create_info = vk::SemaphoreCreateInfo {
    s_type: vk::StructureType::SEMAPHORE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::SemaphoreCreateFlags::empty(),
  };
  unsafe {
    device
      .create_semaphore(&create_info, None)
      .expect("Failed to create a semaphore")
  }
}

fn create_fence(device: &ash::Device) -> vk::Fence {
  let create_info = vk::FenceCreateInfo {
    s_type: vk::StructureType::FENCE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::FenceCreateFlags::empty(),
  };
  unsafe {
    device
      .create_fence(&create_info, None)
      .expect("Failed to create a fence")
  }
}

fn main() {
  env_logger::init();

  let entry: ash::Entry = unsafe { entry::get_entry() };

  #[cfg(feature = "vl")]
  let (instance, mut debug_utils) = instance::create_instance(&entry);
  #[cfg(not(feature = "vl"))]
  let instance = instance::create_instance(&entry);

  let physical_device = unsafe { PhysicalDevice::select(&instance) };

  let (device, queues) = device::create_logical_device(&instance, &physical_device);

  println!("Allocating images...");
  // GPU image with DEVICE_LOCAL flags
  let mut local_image = Image::new(
    &device,
    &physical_device,
    vk::ImageTiling::OPTIMAL,
    vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::COLOR_ATTACHMENT),
    vk::MemoryPropertyFlags::DEVICE_LOCAL,
    vk::MemoryPropertyFlags::empty(),
  );
  // CPU accessible image with HOST_VISIBLE flags
  let mut host_image = Image::new(
    &device,
    &physical_device,
    vk::ImageTiling::LINEAR,
    vk::ImageUsageFlags::TRANSFER_DST,
    vk::MemoryPropertyFlags::HOST_VISIBLE,
    vk::MemoryPropertyFlags::HOST_CACHED,
  );

  let render_pass = create_render_pass(&device);

  let image_view = local_image.create_view(&device);
  let extent = vk::Extent2D {
    width: IMAGE_WIDTH,
    height: IMAGE_HEIGHT,
  };
  let framebuffer = create_framebuffer(&device, render_pass, image_view, extent);

  log::info!("Creating pipeline cache");
  let (pipeline_cache, created_from_file) =
    pipeline_cache::create_pipeline_cache(&device, &physical_device);
  if created_from_file {
    log::info!("Cache successfully created from an existing cache file");
  } else {
    log::info!("Cache initialized as empty");
  }

  log::debug!("Creating pipeline");
  let mut pipeline = GraphicsPipeline::create(&device, pipeline_cache, render_pass);

  // no more pipelines will be created, so might as well save and delete the cache
  log::info!("Saving pipeline cache");
  if let Err(err) = pipeline_cache::save_pipeline_cache(&device, &physical_device, pipeline_cache) {
    log::error!("Failed to save pipeline cache: {:?}", err);
  }
  unsafe {
    device.destroy_pipeline_cache(pipeline_cache, None);
  }

  let mut graphics_pool =
    GraphicsCommandBufferPool::create(&device, &physical_device.queue_families);
  let mut transfer_pool =
    TransferCommandBufferPool::create(&device, &physical_device.queue_families);

  let mut buffers = ConstantBuffers::new(&device, &physical_device, &queues, &mut transfer_pool);

  // record command buffers
  unsafe {
    graphics_pool.reset(&device);
    graphics_pool.record(
      &device,
      &physical_device.queue_families,
      render_pass,
      framebuffer,
      &pipeline,
      &buffers,
      *local_image,
    );

    transfer_pool.reset(&device);
    transfer_pool.record_copy_img_to_host(
      &device,
      &physical_device.queue_families,
      *local_image,
      *host_image,
    );
  }

  let triangle_finished = create_semaphore(&device);
  let triangle_submit = vk::SubmitInfo {
    s_type: vk::StructureType::SUBMIT_INFO,
    p_next: ptr::null(),
    wait_semaphore_count: 0,
    p_wait_semaphores: ptr::null(),
    p_wait_dst_stage_mask: ptr::null(),
    command_buffer_count: 1,
    p_command_buffers: addr_of!(graphics_pool.triangle),
    signal_semaphore_count: 1,
    p_signal_semaphores: addr_of!(triangle_finished),
  };
  let wait_for = vk::PipelineStageFlags::TRANSFER;
  let transfer_image_submit = vk::SubmitInfo {
    s_type: vk::StructureType::SUBMIT_INFO,
    p_next: ptr::null(),
    wait_semaphore_count: 1,
    p_wait_semaphores: addr_of!(triangle_finished),
    p_wait_dst_stage_mask: addr_of!(wait_for),
    command_buffer_count: 1,
    p_command_buffers: addr_of!(transfer_pool.copy_to_host),
    signal_semaphore_count: 0,
    p_signal_semaphores: ptr::null(),
  };

  let finished = create_fence(&device);

  println!("Submitting work...");
  unsafe {
    device
      .queue_submit(queues.graphics, &[triangle_submit], vk::Fence::null())
      .expect("Failed to submit compute");
    device
      .queue_submit(queues.transfer, &[transfer_image_submit], finished)
      .expect("Failed to submit transfer");

    device
      .wait_for_fences(&[finished], true, u64::MAX)
      .expect("Failed to wait for fences");
  }
  println!("GPU finished!");

  println!("Saving file...");
  host_image.save_to_file(&device, &physical_device, IMAGE_SAVE_PATH);
  println!("Done!");

  // Cleanup
  log::info!("Destroying and releasing resources");
  unsafe {
    // wait until all operations have finished and the device is safe to destroy
    device
      .device_wait_idle()
      .expect("Failed to wait for the device to become idle");

    device.destroy_fence(finished, None);
    device.destroy_semaphore(triangle_finished, None);

    device.destroy_framebuffer(framebuffer, None);
    device.destroy_image_view(image_view, None);
    device.destroy_render_pass(render_pass, None);

    pipeline.destroy_self(&device);

    graphics_pool.destroy_self(&device);
    transfer_pool.destroy_self(&device);

    buffers.destroy_self(&device);

    local_image.destroy_self(&device);
    host_image.destroy_self(&device);

    log::debug!("Destroying device");
    device.destroy_device(None);

    #[cfg(feature = "vl")]
    {
      log::debug!("Destroying debug utils messenger");
      debug_utils.destroy_self();
    }

    log::debug!("Destroying Instance");
    instance.destroy_instance(None);
  }
}
