use std::ptr;

use ash::vk;
use log::debug;

use crate::{physical_device::PhysicalDevice, IMG_HEIGHT, IMG_WIDTH};

pub struct Image {
  pub vk_img: vk::Image,
  pub memory: vk::DeviceMemory,
  pub memory_type_i: u32,
  pub memory_size: u64,
}

impl Image {
  pub fn new(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    tiling: vk::ImageTiling,
    usage: vk::ImageUsageFlags,
    required_memory_properties: vk::MemoryPropertyFlags,
    optional_memory_properties: vk::MemoryPropertyFlags,
  ) -> Self {
    debug!("Creating image");
    let vk_img = create_img(device, tiling, usage);

    debug!("Allocating memory for image");
    let (memory, memory_type_i, memory_size) = allocate_img_memory(
      device,
      physical_device,
      vk_img,
      required_memory_properties,
      optional_memory_properties,
    );

    debug!("Binding memory to image");
    unsafe {
      device
        .bind_image_memory(vk_img, memory, 0)
        .expect("Failed to bind memory to image")
    };

    Self {
      vk_img,
      memory,
      memory_type_i,
      memory_size,
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_image(self.vk_img, None);
    device.free_memory(self.memory, None);
  }
}

fn create_img(
  device: &ash::Device,
  tiling: vk::ImageTiling,
  usage: vk::ImageUsageFlags,
) -> vk::Image {
  let create_info = vk::ImageCreateInfo {
    s_type: vk::StructureType::IMAGE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::ImageCreateFlags::empty(),
    image_type: vk::ImageType::TYPE_2D,
    format: vk::Format::R8G8B8A8_UINT,
    extent: vk::Extent3D {
      width: IMG_WIDTH,
      height: IMG_HEIGHT,
      depth: 1,
    },
    mip_levels: 1,
    array_layers: 1,
    samples: vk::SampleCountFlags::TYPE_1,
    tiling,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(), // ignored if sharing mode is concurrent
    initial_layout: vk::ImageLayout::UNDEFINED,
  };

  unsafe {
    device
      .create_image(&create_info, None)
      .expect("Failed to create image")
  }
}

// usually you will have multiple images of similar type that are going to use only one memory allocation
fn allocate_img_memory(
  device: &ash::Device,
  physical_device: &PhysicalDevice,
  image: vk::Image,
  required_memory_properties: vk::MemoryPropertyFlags,
  optional_memory_properties: vk::MemoryPropertyFlags,
) -> (vk::DeviceMemory, u32, u64) {
  let memory_requirements = unsafe { device.get_image_memory_requirements(image) };

  // Try to find optimal memory type. If it doesn't exist, try to find required memory type
  let memory_type = physical_device
    .find_optimal_memory_type(
      memory_requirements.memory_type_bits,
      required_memory_properties,
      optional_memory_properties,
    )
    .expect("Failed to find appropriate memory type to allocate an image");

  let allocate_info = vk::MemoryAllocateInfo {
    s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
    p_next: ptr::null(),
    allocation_size: memory_requirements.size,
    memory_type_index: memory_type,
  };

  let memory = unsafe {
    device
      .allocate_memory(&allocate_info, None)
      .expect("Failed to allocate memory for an image")
  };

  (memory, memory_type, memory_requirements.size)
}
