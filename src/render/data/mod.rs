pub mod compute;
pub mod constant;

use std::ptr::NonNull;

use ash::vk;

use crate::utility::const_flag_bitor;

use super::{
  device_destroyable::DeviceManuallyDestroyed,
  render_object::{QUAD_INDICES, QUAD_INDICES_SIZE, QUAD_VERTICES_SIZE},
};

pub const VERTEX_SIZE: u64 = QUAD_VERTICES_SIZE as u64;
pub const INDEX_SIZE: u64 = QUAD_INDICES_SIZE as u64;
pub const INDEX_COUNT: u32 = QUAD_INDICES.len() as u32;

const TEXTURE_PATH: &str = "./ferris.png";
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

impl<T> DeviceManuallyDestroyed for MappedHostBuffer<T> {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.buffer.destroy_self(device);
  }
}
