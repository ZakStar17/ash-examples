mod allocator;
mod command_pools;
mod device;
mod entry;
mod image;
mod instance;
mod utility;

// validation layers module will only exist if validation layers are enabled
#[cfg(feature = "vl")]
mod validation_layers;

use ash::vk;
use command_pools::{ComputeCommandBufferPool, TransferCommandBufferPool};
use device::PhysicalDevice;
use std::{
  ffi::CStr,
  ops::BitOr,
  ptr::{self, addr_of},
};

use crate::{
  allocator::allocate_and_bind_memory,
  image::{create_image, save_buffer_to_image_file},
};

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
  float32: [134.0 / 255.0, 206.0 / 255.0, 203.0 / 255.0, 1.0] , // rgba(134, 206, 203, 255)
};

const IMAGE_SAVE_PATH: &str = "image.png";

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

fn create_buffer(
  device: &ash::Device,
  size: u64,
  usage: vk::BufferUsageFlags,
) -> Result<vk::Buffer, vk::Result> {
  let create_info = vk::BufferCreateInfo {
    s_type: vk::StructureType::BUFFER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::BufferCreateFlags::empty(),
    size,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(),
  };
  unsafe { device.create_buffer(&create_info, None) }
}

fn main() {
  env_logger::init();

  let entry: ash::Entry = unsafe { entry::get_entry() };

  #[cfg(feature = "vl")]
  let (instance, mut debug_utils) =
    instance::create_instance(&entry).expect("Failed to create an instance");
  #[cfg(not(feature = "vl"))]
  let instance = instance::create_instance(&entry).expect("Failed to create an instance");

  let physical_device = match unsafe { PhysicalDevice::select(&instance) } {
    Ok(device_opt) => match device_opt {
      Some(device) => device,
      None => panic!("No suitable device found"),
    },
    Err(err) => panic!("Failed to query physical devices: {:?}", err),
  };

  let (device, queues) = device::create_logical_device(&instance, &physical_device)
    .expect("Failed to create logical device");

  // GPU image with DEVICE_LOCAL flags
  let local_image = create_image(
    &device,
    vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::TRANSFER_DST),
  )
  .expect("Failed to create image");
  let local_image_memory = allocate_and_bind_memory(
    &device,
    &physical_device,
    vk::MemoryPropertyFlags::DEVICE_LOCAL,
    &[],
    &[],
    &[local_image],
    &[unsafe { device.get_image_memory_requirements(local_image) }],
  )
  .or_else(|err| {
    log::warn!(
      "Failed to allocate optimal memory for image: {:?}\nTrying to allocate suboptimally",
      err
    );
    allocate_and_bind_memory(
      &device,
      &physical_device,
      vk::MemoryPropertyFlags::empty(),
      &[],
      &[],
      &[local_image],
      &[unsafe { device.get_image_memory_requirements(local_image) }],
    )
  })
  .expect("Failed to allocate memory for image")
  .memory;

  let buffer_size = IMAGE_WIDTH as u64 * IMAGE_HEIGHT as u64 * 4;
  let host_buffer = create_buffer(&device, buffer_size, vk::BufferUsageFlags::TRANSFER_DST)
    .expect("Failed to create buffer");

  let host_buffer_memory = allocate_and_bind_memory(
    &device,
    &physical_device,
    vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_CACHED),
    &[host_buffer],
    &[unsafe { device.get_buffer_memory_requirements(host_buffer) }],
    &[],
    &[],
  )
  .or_else(|err| {
    log::warn!(
      "Failed to allocate optimal memory for buffer: {:?}\nTrying to allocate suboptimally",
      err
    );
    allocate_and_bind_memory(
      &device,
      &physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE,
      &[host_buffer],
      &[unsafe { device.get_buffer_memory_requirements(host_buffer) }],
      &[],
      &[],
    )
  })
  .expect("Failed to allocate memory for buffer")
  .memory;

  let mut compute_pool = ComputeCommandBufferPool::create(&device, &physical_device.queue_families);
  let mut transfer_pool =
    TransferCommandBufferPool::create(&device, &physical_device.queue_families);

  // record command buffers
  unsafe {
    compute_pool.reset(&device);
    compute_pool.record_clear_img(&device, &physical_device.queue_families, local_image);

    transfer_pool.reset(&device);
    transfer_pool.record_copy_img_to_buffer(
      &device,
      &physical_device.queue_families,
      local_image,
      host_buffer,
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
    p_command_buffers: addr_of!(compute_pool.clear_img),
    signal_semaphore_count: 1,
    p_signal_semaphores: addr_of!(image_clear_finished),
  };
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

  let finished = create_fence(&device);

  println!("Submitting work...");
  unsafe {
    // note: you can make multiple submits with device.queue_submit2
    device
      .queue_submit(queues.compute, &[clear_image_submit], vk::Fence::null())
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
  save_buffer_to_image_file(
    &device,
    host_buffer_memory,
    buffer_size as usize,
    IMAGE_SAVE_PATH,
  );
  println!("Done!");

  // Cleanup
  log::info!("Destroying and releasing resources");
  unsafe {
    // wait until all operations have finished and the device is safe to destroy
    device
      .device_wait_idle()
      .expect("Failed to wait for the device to become idle");

    device.destroy_fence(finished, None);
    device.destroy_semaphore(image_clear_finished, None);

    compute_pool.destroy_self(&device);
    transfer_pool.destroy_self(&device);

    device.destroy_image(local_image, None);
    device.free_memory(local_image_memory, None);
    device.destroy_buffer(host_buffer, None);
    device.free_memory(host_buffer_memory, None);

    log::debug!("Destroying device");
    device.destroy_device(None);

    #[cfg(feature = "vl")]
    {
      debug_utils.destroy_self();
    }
    instance.destroy_instance(None);
  }
}
