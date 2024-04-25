use std::{marker::PhantomData, ptr};

use ash::vk;

use crate::{errors::OutOfMemoryError, IMAGE_FORMAT};

pub fn create_semaphore(device: &ash::Device) -> Result<vk::Semaphore, OutOfMemoryError> {
  let create_info = vk::SemaphoreCreateInfo::default();
  unsafe { device.create_semaphore(&create_info, None) }.map_err(|err| err.into())
}

pub fn create_fence(device: &ash::Device) -> Result<vk::Fence, OutOfMemoryError> {
  let create_info = vk::FenceCreateInfo::default();
  unsafe { device.create_fence(&create_info, None) }.map_err(|err| err.into())
}

pub fn create_buffer(
  device: &ash::Device,
  size: u64,
  usage: vk::BufferUsageFlags,
) -> Result<vk::Buffer, OutOfMemoryError> {
  let create_info = vk::BufferCreateInfo {
    s_type: vk::StructureType::BUFFER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::BufferCreateFlags::empty(),
    size,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(),
    _marker: PhantomData,
  };
  unsafe { device.create_buffer(&create_info, None) }.map_err(|err| err.into())
}

pub fn create_image(
  device: &ash::Device,
  width: u32,
  height: u32,
  usage: vk::ImageUsageFlags,
) -> Result<vk::Image, OutOfMemoryError> {
  // 1 color layer 2d image
  let create_info = vk::ImageCreateInfo {
    s_type: vk::StructureType::IMAGE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::ImageCreateFlags::empty(),
    image_type: vk::ImageType::TYPE_2D,
    format: IMAGE_FORMAT,
    extent: vk::Extent3D {
      width,
      height,
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
    _marker: PhantomData,
  };

  unsafe { device.create_image(&create_info, None) }.map_err(|err| err.into())
}
