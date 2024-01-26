mod command_pool;
mod entry;
mod image;
mod instance;
mod logical_device;
mod physical_device;
mod utility;

// validation layers module will only exist if validation layers are enabled
#[cfg(feature = "vl")]
mod validation_layers;

use ash::vk;
use command_pool::ComputeCommandBufferPool;
use image::Image;
use physical_device::PhysicalDevice;
use std::{ffi::CStr, ops::BitOr, ptr};

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

// Vulkan API version required to run the program
// In your case you may request a optimal version of the API in order to use specific features
// but fallback to an older version if the target is not supported by the driver or any physical
// device
pub const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

// somewhat arbitrary
pub const APPLICATION_NAME: &'static CStr = cstr!("Vulkan Instance creation");
pub const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

pub const REQUIRED_DEVICE_EXTENSIONS: [&'static CStr; 0] = [];

pub const IMG_WIDTH: u32 = 800;
pub const IMG_HEIGHT: u32 = 800;

fn main() {
  env_logger::init();

  let entry: ash::Entry = unsafe { entry::get_entry() };

  #[cfg(feature = "vl")]
  let (instance, mut debug_utils) = instance::create_instance(&entry);
  #[cfg(not(feature = "vl"))]
  let instance = instance::create_instance(&entry);

  let physical_device = unsafe { PhysicalDevice::select(&instance) };

  let (device, queues) = logical_device::create_logical_device(&instance, &physical_device);

  let mut local_image = Image::new(
    &device,
    &physical_device,
    vk::ImageTiling::OPTIMAL,
    vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::TRANSFER_DST),
    vk::MemoryPropertyFlags::DEVICE_LOCAL,
    vk::MemoryPropertyFlags::empty(),
  );
  let mut host_image = Image::new(
    &device,
    &physical_device,
    vk::ImageTiling::LINEAR,
    vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::TRANSFER_DST),
    vk::MemoryPropertyFlags::HOST_VISIBLE,
    vk::MemoryPropertyFlags::HOST_CACHED,
  );

  let mut compute_pool = ComputeCommandBufferPool::create(&device, &physical_device.queue_families);

  unsafe {
    compute_pool.reset(&device);
    compute_pool.record_clear_img(&device, &physical_device.queue_families, local_image.vk_img);
  }

  let command_buffers = [compute_pool.clear_img];
  let submit_info = vk::SubmitInfo {
    s_type: vk::StructureType::SUBMIT_INFO,
    p_next: ptr::null(),
    wait_semaphore_count: 0,
    p_wait_semaphores: ptr::null(),
    p_wait_dst_stage_mask: ptr::null(),
    command_buffer_count: command_buffers.len() as u32,
    p_command_buffers: command_buffers.as_ptr(),
    signal_semaphore_count: 0,
    p_signal_semaphores: ptr::null(),
  };

  unsafe {
    device
      .queue_submit(queues.compute, &[submit_info], vk::Fence::null())
      .expect("Failed to submit compute");
  }

  // Cleanup
  unsafe {
    // wait until all operations have finished and the device is safe to destroy
    device
      .device_wait_idle()
      .expect("Failed to wait for the device to become idle");

    log::debug!("Destroying command pool");
    compute_pool.destroy_self(&device);

    local_image.destroy_self(&device);
    host_image.destroy_self(&device);

    // destroying a logical device also implicitly destroys all associated queues
    log::debug!("Destroying logical device");
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
