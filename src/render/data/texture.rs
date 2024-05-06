use std::ptr::copy_nonoverlapping;

use ash::vk;

use crate::{
  const_flag_bitor,
  render::{
    create_objs::{create_buffer, create_image, create_image_view},
    device_destroyable::DeviceManuallyDestroyed,
    errors::OutOfMemoryError,
  },
};

use super::StagingMemoryAllocation;

const TEXTURE_PATH: &'static str = "./ferris.png";
pub const TEXTURE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
pub const TEXTURE_USAGES: vk::ImageUsageFlags = const_flag_bitor!(
  vk::ImageUsageFlags,
  vk::ImageUsageFlags::SAMPLED,
  vk::ImageUsageFlags::TRANSFER_DST
);

fn read_texture_bytes_as_rgba8() -> Result<(u32, u32, Vec<u8>), image::ImageError> {
  let img = image::io::Reader::open(TEXTURE_PATH)?
    .decode()?
    .into_rgba8();
  let width = img.width();
  let height = img.height();

  let bytes = img.into_raw();
  assert!(bytes.len() == width as usize * height as usize * 4);
  Ok((width, height, bytes))
}

pub struct LoadedImage {
  pub image: vk::Image,
  pub width: u32,
  pub height: u32,
  pub bytes: Box<[u8]>,
}

impl DeviceManuallyDestroyed for LoadedImage {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.image.destroy_self(device);
  }
}

#[derive(Debug, thiserror::Error)]
pub enum ImageLoadError {
  #[error("Out of memory")]
  OutOfMemory(#[source] OutOfMemoryError),
  #[error("Image crate error")]
  ImageError(#[source] image::ImageError),
}

pub struct Texture {
  pub image: vk::Image,
  pub memory: vk::DeviceMemory, // not owned
  pub view: vk::ImageView,
  pub descriptor: vk::DescriptorSet,
}

#[derive(Debug, thiserror::Error)]
pub enum ImageCreationError {
  #[error("Out of memory")]
  OutOfMemoryError(#[source] OutOfMemoryError),
  #[error("No supported formats available")]
  NoSupportedFormats,
}

impl Texture {
  // takes image ownership
  // other objects must be managed outside
  pub fn new(
    device: &ash::Device,
    image: vk::Image,
    memory: vk::DeviceMemory,
    descriptor: vk::DescriptorSet,
  ) -> Result<Self, OutOfMemoryError> {
    let view = create_image_view(device, image, TEXTURE_FORMAT)?;
    Ok(Self {
      image,
      memory,
      view,
      descriptor,
    })
  }

  pub fn create_image(device: &ash::Device) -> Result<LoadedImage, ImageLoadError> {
    let (width, height, bytes) =
      read_texture_bytes_as_rgba8().map_err(|err| ImageLoadError::ImageError(err))?;
    let image = create_image(device, TEXTURE_FORMAT, width, height, TEXTURE_USAGES)
      .map_err(|err| ImageLoadError::OutOfMemory(err))?;
    Ok(LoadedImage {
      image,
      width,
      height,
      bytes: bytes.into_boxed_slice(),
    })
  }

  pub fn create_staging_buffer(
    device: &ash::Device,
    image: &LoadedImage,
  ) -> Result<vk::Buffer, OutOfMemoryError> {
    create_buffer(
      device,
      image.bytes.len() as u64,
      vk::BufferUsageFlags::TRANSFER_SRC,
    )
  }

  pub unsafe fn populate_staging_buffer(
    mem_ptr: *mut u8,
    alloc: StagingMemoryAllocation,
    buffer_bytes: &[u8],
  ) {
    copy_nonoverlapping(
      buffer_bytes.as_ptr(),
      mem_ptr.byte_add(alloc.texture_offset as usize) as *mut u8,
      buffer_bytes.len(),
    );
  }
}

impl DeviceManuallyDestroyed for Texture {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.view.destroy_self(device);
    self.image.destroy_self(device);
  }
}
