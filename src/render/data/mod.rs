pub mod compute;
pub mod constant;
mod screenshot_buffer;

use std::{ops::Deref, ptr::NonNull};

use ash::vk;

use crate::utility::const_flag_bitor;

use super::device_destroyable::DeviceManuallyDestroyed;

pub use screenshot_buffer::ScreenshotBuffer;

const TEXTURE_PATH: &str = "./sprites.png";
pub const TEXTURE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
pub const TEXTURE_USAGES: vk::ImageUsageFlags = const_flag_bitor!(
  vk::ImageUsageFlags =>
  vk::ImageUsageFlags::SAMPLED,
  vk::ImageUsageFlags::TRANSFER_DST
);
pub const TEXTURE_FORMAT_FEATURES: vk::FormatFeatureFlags = const_flag_bitor!(
  vk::FormatFeatureFlags =>
  vk::FormatFeatureFlags::TRANSFER_DST,
  vk::FormatFeatureFlags::SAMPLED_IMAGE
);

// buffer and its mapped ptr
#[derive(Debug)]
pub struct MappedHostBuffer<T> {
  pub buffer: vk::Buffer,
  pub data_ptr: NonNull<T>,
}

impl<T> Deref for MappedHostBuffer<T> {
  type Target = vk::Buffer;

  fn deref(&self) -> &Self::Target {
    &self.buffer
  }
}

impl<T> DeviceManuallyDestroyed for MappedHostBuffer<T> {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.buffer.destroy_self(device);
  }
}
