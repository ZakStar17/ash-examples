mod command_pools;
mod entry;
mod image;
mod instance;
mod device;
mod utility;

// validation layers module will only exist if validation layers are enabled
#[cfg(feature = "vl")]
mod validation_layers;

use ash::vk;
use command_pools::{ComputeCommandBufferPool, TransferCommandBufferPool};
use image::Image;
use device::PhysicalDevice;
use std::{
  ffi::CStr,
  ops::BitOr,
  ptr::{self, addr_of},
};

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

// try putting some very big numbers
pub const IMAGE_WIDTH: u32 = 1920;
pub const IMAGE_HEIGHT: u32 = 1080;

// some formats may not be available
pub const IMAGE_FORMAT: vk::Format = vk::Format::R8G8B8A8_UINT;
// color value depends on format to be applied correctly
pub const IMAGE_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  uint32: [134, 206, 203, 255], // rgba(134, 206, 203, 255)
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
  let mut local_image = Image::new(
    &device,
    &physical_device,
    vk::ImageTiling::OPTIMAL,
    vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::TRANSFER_DST),
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

  let mut compute_pool = ComputeCommandBufferPool::create(&device, &physical_device.queue_families);
  let mut transfer_pool =
    TransferCommandBufferPool::create(&device, &physical_device.queue_families);

  // record command buffers
  unsafe {
    compute_pool.reset(&device);
    compute_pool.record_clear_img(&device, &physical_device.queue_families, *local_image);

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

  // create a fence to signal when all operation have finished
  let operation_finished = create_fence(&device);

  println!("Submitting work...");
  unsafe {
    // note: you can make multiple submits with device.queue_submit2
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

    log::debug!("Destroying fence");
    device.destroy_fence(operation_finished, None);

    log::debug!("Destroying semaphore");
    device.destroy_semaphore(image_clear_finished, None);

    log::debug!("Destroying command pools");
    compute_pool.destroy_self(&device);
    transfer_pool.destroy_self(&device);

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
