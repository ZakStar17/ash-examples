use std::ptr;

use ash::vk;

use crate::{IMAGE_FORMAT, IMAGE_HEIGHT, IMAGE_WIDTH};

pub fn create_image(
  device: &ash::Device,
  usage: vk::ImageUsageFlags,
) -> Result<vk::Image, vk::Result> {
  // 1 color layer 2d image
  let create_info = vk::ImageCreateInfo {
    s_type: vk::StructureType::IMAGE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::ImageCreateFlags::empty(),
    image_type: vk::ImageType::TYPE_2D,
    format: IMAGE_FORMAT,
    extent: vk::Extent3D {
      width: IMAGE_WIDTH,
      height: IMAGE_HEIGHT,
      depth: 1,
    },
    mip_levels: 1,
    array_layers: 1,
    samples: vk::SampleCountFlags::TYPE_1,
    tiling: vk::ImageTiling::OPTIMAL,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(), // ignored if sharing mode is exclusive
    initial_layout: vk::ImageLayout::UNDEFINED,
  };

  unsafe { device.create_image(&create_info, None) }
}
