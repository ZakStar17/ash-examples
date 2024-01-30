#![feature(offset_of)]

mod command_pools;
mod descriptor_sets;
mod device;
mod entry;
mod image;
mod instance;
mod pipeline;
mod pipeline_cache;
mod shaders;
mod utility;

// validation layers module will only exist if validation layers are enabled
#[cfg(feature = "vl")]
mod validation_layers;

use ash::vk;
use command_pools::{ComputeCommandBufferPool, TransferCommandBufferPool};
use device::PhysicalDevice;
use image::Image;
use std::{
  ffi::CStr,
  ops::BitOr,
  ptr::{self, addr_of},
};

use crate::{descriptor_sets::DescriptorSets, pipeline::ComputePipeline};

// simple macro to transmute literals to static CStr
macro_rules! cstr {
  ( $s:literal ) => {{
    unsafe { std::mem::transmute::<_, &CStr>(concat!($s, "\0")) }
  }};
}

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

pub const IMAGE_WIDTH: u32 = 4000;
pub const IMAGE_HEIGHT: u32 = 4000;

// what is used in the shader
pub const IMAGE_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;

// Size of each local group in the shader invocation
// Normally these would be calculated from image dimensions and clapped to respect device limits
// but for this example a small local group size should suffice (limits are still checked in
// physical device selection)
pub const SHADER_GROUP_SIZE_X: u32 = 16;
pub const SHADER_GROUP_SIZE_Y: u32 = 16;

// mandelbrot constants
// these are passed as specialization constants in the shader
pub const MAX_ITERATIONS: u32 = 10000;
pub const FOCAL_POINT: [f32; 2] = [-0.765, 0.0]; // complex plane coordinates of the image center
pub const ZOOM: f32 = 0.40486;

const IMAGE_SAVE_PATH: &str = "./image.png";

fn create_sampler(device: &ash::Device) -> vk::Sampler {
  let sampler_create_info = vk::SamplerCreateInfo {
    s_type: vk::StructureType::SAMPLER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::SamplerCreateFlags::empty(),
    mag_filter: vk::Filter::NEAREST,
    min_filter: vk::Filter::NEAREST,
    address_mode_u: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    address_mode_v: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    address_mode_w: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    anisotropy_enable: vk::FALSE,
    max_anisotropy: 0.0,
    border_color: vk::BorderColor::INT_OPAQUE_BLACK,
    unnormalized_coordinates: vk::TRUE,
    compare_enable: vk::FALSE,
    compare_op: vk::CompareOp::NEVER,
    mipmap_mode: vk::SamplerMipmapMode::NEAREST,
    mip_lod_bias: 0.0,
    max_lod: 0.0,
    min_lod: 0.0,
  };
  unsafe {
    device
      .create_sampler(&sampler_create_info, None)
      .expect("Failed to create a sampler")
  }
}

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

  // GPU image with DEVICE_LOCAL flags
  println!("Allocating images...");
  let mut local_image = Image::new(
    &device,
    &physical_device,
    vk::ImageTiling::OPTIMAL,
    vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::STORAGE),
    vk::MemoryPropertyFlags::DEVICE_LOCAL,
    vk::MemoryPropertyFlags::empty(),
  );
  // CPU accessible image with HOST_VISIBLE flags
  let mut host_image = Image::new(
    &device,
    &physical_device,
    vk::ImageTiling::LINEAR,
    vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::TRANSFER_DST),
    vk::MemoryPropertyFlags::HOST_VISIBLE,
    vk::MemoryPropertyFlags::HOST_CACHED,
  );

  // the sampler technically is useless as the image is never used as a sampled image, however
  // it still needs to be passed to the write descriptor set
  let sampler = create_sampler(&device);
  let image_view = local_image.create_view(&device);
  let mut descriptor_sets = DescriptorSets::new(&device);
  descriptor_sets
    .pool
    .write_image(&device, image_view, sampler);

  log::info!("Creating pipeline cache");
  let (pipeline_cache, created_from_file) =
    pipeline_cache::create_pipeline_cache(&device, &physical_device);
  if created_from_file {
    log::info!("Cache successfully created from an existing cache file");
  } else {
    log::info!("Cache initialized as empty");
  }

  log::debug!("Creating pipeline");
  let mut pipeline = ComputePipeline::create(&device, pipeline_cache, &descriptor_sets);

  // no more pipelines will be created, so might as well save and delete the cache
  log::info!("Saving pipeline cache");
  if let Err(err) = pipeline_cache::save_pipeline_cache(&device, &physical_device, pipeline_cache) {
    log::error!("Failed to save pipeline cache: {:?}", err);
  }
  unsafe {
    device.destroy_pipeline_cache(pipeline_cache, None);
  }

  let mut compute_pool = ComputeCommandBufferPool::create(&device, &physical_device.queue_families);
  let mut transfer_pool =
    TransferCommandBufferPool::create(&device, &physical_device.queue_families);

  // record command buffers
  unsafe {
    compute_pool.reset(&device);
    compute_pool.record_mandelbrot(
      &device,
      &physical_device.queue_families,
      &pipeline,
      &descriptor_sets,
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

  let image_clear_finished = create_semaphore(&device);
  let clear_image_submit = vk::SubmitInfo {
    s_type: vk::StructureType::SUBMIT_INFO,
    p_next: ptr::null(),
    wait_semaphore_count: 0,
    p_wait_semaphores: ptr::null(),
    p_wait_dst_stage_mask: ptr::null(),
    command_buffer_count: 1,
    p_command_buffers: addr_of!(compute_pool.storage_image),
    signal_semaphore_count: 1,
    p_signal_semaphores: addr_of!(image_clear_finished),
  };
  // compute_pool.storage_image last pipeline barriers makes sure that all operations finish before
  // TRANSFER, so that's the dst_mask for the semaphore
  // It cannot be COMPUTE_SHADER as the transfer queue cannot use it as its src_mask
  let wait_for = vk::PipelineStageFlags::TRANSFER;
  let transfer_image_submit = vk::SubmitInfo {
    s_type: vk::StructureType::SUBMIT_INFO,
    p_next: ptr::null(),
    wait_semaphore_count: 1,
    p_wait_semaphores: addr_of!(image_clear_finished),
    p_wait_dst_stage_mask: addr_of!(wait_for),
    command_buffer_count: 1,
    p_command_buffers: addr_of!(transfer_pool.copy_to_host),
    signal_semaphore_count: 0,
    p_signal_semaphores: ptr::null(),
  };

  let operation_finished = create_fence(&device);

  println!("Submitting work...");
  unsafe {
    device
      .queue_submit(queues.compute, &[clear_image_submit], vk::Fence::null())
      .expect("Failed to submit compute");
    device
      .queue_submit(
        queues.transfer,
        &[transfer_image_submit],
        operation_finished,
      )
      .expect("Failed to submit transfer");
    device
      .wait_for_fences(&[operation_finished], true, u64::MAX)
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

    device.destroy_fence(operation_finished, None);
    device.destroy_semaphore(image_clear_finished, None);

    compute_pool.destroy_self(&device);
    transfer_pool.destroy_self(&device);

    pipeline.destroy_self(&device);
    descriptor_sets.destroy_self(&device);

    device.destroy_image_view(image_view, None);

    local_image.destroy_self(&device);
    host_image.destroy_self(&device);

    device.destroy_sampler(sampler, None);

    device.destroy_device(None);

    #[cfg(feature = "vl")]
    {
      debug_utils.destroy_self();
    }

    instance.destroy_instance(None);
  }
}
