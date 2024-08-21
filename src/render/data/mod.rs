mod constant;
mod staging_constant;

use std::{
  marker::PhantomData,
  ptr::{self},
};

use ash::vk;
use staging_constant::StagingData;

use crate::{
  render::{
    command_pools::TransferCommandBufferPool,
    create_objs::create_fence,
    device_destroyable::{destroy, DeviceManuallyDestroyed},
    errors::OutOfMemoryError,
    initialization::device::{PhysicalDevice, Queues},
  },
  utility::{const_flag_bitor, OnErr},
};

use super::{
  command_pools::TemporaryGraphicsCommandPool,
  create_objs::create_semaphore,
  errors::InitializationError,
  initialization::device::Device,
  render_object::{QUAD_INDICES, QUAD_INDICES_SIZE, QUAD_VERTICES_SIZE},
};

pub use constant::ConstantData;

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

#[derive(Debug, thiserror::Error)]
pub enum ImageLoadError {
  #[error("Out of memory")]
  OutOfMemory(#[source] OutOfMemoryError),
  #[error("Image crate error")]
  ImageError(#[source] image::ImageError),
}

fn read_texture_bytes_as_rgba8() -> Result<(u32, u32, Vec<u8>), image::ImageError> {
  let img = image::ImageReader::open(TEXTURE_PATH)?
    .decode()?
    .into_rgba8();
  let width = img.width();
  let height = img.height();

  let bytes = img.into_raw();
  assert!(bytes.len() == width as usize * height as usize * 4);
  Ok((width, height, bytes))
}

pub fn create_and_populate_constant_data(
  device: &Device,
  physical_device: &PhysicalDevice,
  queues: &Queues,
  transfer_pool: &mut TransferCommandBufferPool,
  graphics_pool: &mut TemporaryGraphicsCommandPool,
) -> Result<ConstantData, InitializationError> {
  let (image_width, image_height, image_bytes) =
    read_texture_bytes_as_rgba8().map_err(ImageLoadError::ImageError)?;

  let staging =
    StagingData::create_and_allocate(device, physical_device, image_bytes.len() as u64)?;
  unsafe { staging.populate(device, &image_bytes) }
    .on_err(|_| unsafe { staging.destroy_self(device) });

  let final_ =
    ConstantData::create_and_allocate(device, physical_device, image_width, image_height)?;

  let vertex_region = vk::BufferCopy2::default().size(VERTEX_SIZE);
  let index_region = vk::BufferCopy2::default().size(INDEX_SIZE);
  unsafe {
    transfer_pool.reset(device)?;
    transfer_pool.record_copy_buffers_to_buffers_from_host(
      device,
      &[
        vk::CopyBufferInfo2 {
          s_type: vk::StructureType::COPY_BUFFER_INFO_2,
          p_next: ptr::null(),
          src_buffer: staging.vertex,
          dst_buffer: final_.vertex,
          region_count: 1,
          p_regions: &vertex_region,
          _marker: PhantomData,
        },
        vk::CopyBufferInfo2 {
          s_type: vk::StructureType::COPY_BUFFER_INFO_2,
          p_next: ptr::null(),
          src_buffer: staging.index,
          dst_buffer: final_.index,
          region_count: 1,
          p_regions: &index_region,
          _marker: PhantomData,
        },
      ],
    )?;

    transfer_pool.record_load_texture(
      device,
      &physical_device.queue_families,
      staging.texture,
      final_.texture,
      image_width,
      image_height,
    )?;

    if physical_device.queue_families.get_graphics_index()
      != physical_device.queue_families.get_transfer_index()
    {
      graphics_pool.reset(device)?;
      graphics_pool.record_acquire_texture(
        device,
        &physical_device.queue_families,
        final_.texture,
      )?;
    }
  }

  let copy_buffers_to_buffers = [transfer_pool.copy_buffers_to_buffers];
  let load_texture = [transfer_pool.load_texture];
  let acquire_texture = [graphics_pool.acquire_texture];

  let ferris_submit_info = vk::SubmitInfo::default().command_buffers(&copy_buffers_to_buffers);

  if physical_device.queue_families.get_graphics_index()
    != physical_device.queue_families.get_transfer_index()
  {
    let texture_finished = create_fence(device)?;
    let ferris_finished =
      create_fence(device).on_err(|_| unsafe { texture_finished.destroy_self(device) })?;
    let wait_texture_transfer = create_semaphore(device)
      .on_err(|_| unsafe { destroy!(device => &texture_finished, &ferris_finished) })?;
    let destroy_objects = || unsafe {
      destroy!(device => &texture_finished, &ferris_finished, &wait_texture_transfer);
    };

    let wait_texture_transfer_arr = [wait_texture_transfer];
    let texture_submit_info_a = vk::SubmitInfo::default()
      .command_buffers(&load_texture)
      .signal_semaphores(&wait_texture_transfer_arr);
    let texture_submit_info_b = vk::SubmitInfo::default()
      .command_buffers(&acquire_texture)
      .wait_semaphores(&wait_texture_transfer_arr)
      .wait_dst_stage_mask(&[vk::PipelineStageFlags::TRANSFER]);

    unsafe {
      device.queue_submit(queues.transfer, &[ferris_submit_info], ferris_finished)?;
      device.queue_submit(queues.transfer, &[texture_submit_info_a], vk::Fence::null())?;
      device.queue_submit(queues.graphics, &[texture_submit_info_b], texture_finished)?;

      device.wait_for_fences(&[ferris_finished, texture_finished], true, u64::MAX)?;

      destroy_objects();
    }
  } else {
    let all_finished = create_fence(device)?;

    let texture_submit_info = vk::SubmitInfo::default().command_buffers(&load_texture);

    unsafe {
      device.queue_submit(
        queues.graphics,
        &[ferris_submit_info, texture_submit_info],
        all_finished,
      )?;
      device.wait_for_fences(&[all_finished], true, u64::MAX)?;

      all_finished.destroy_self(device);
    }
  }

  unsafe {
    staging.destroy_self(device);
  }

  Ok(final_)
}
