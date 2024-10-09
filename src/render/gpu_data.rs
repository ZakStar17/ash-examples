use std::ops::BitOr;

use ash::vk;

use crate::{
  render::{
    allocator::{self, MemoryWithType},
    command_pools::{self, initialization::PendingInitialization},
    create_objs::{create_buffer, create_image, create_image_view},
    device_destroyable::{destroy, DeviceManuallyDestroyed},
    errors::QueueSubmitError,
    initialization::device::{Device, PhysicalDevice, Queues},
    render_object::{QUAD_INDICES, QUAD_INDICES_SIZE, VERTICES, VERTICES_SIZE},
  },
  utility::{const_flag_bitor, OnErr},
};

use super::allocator::{DeviceMemoryInitializationError, SingleUseStagingBuffers};

const TEXTURE_PATH: &str = "./ferris.png";
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

#[derive(Debug)]
pub struct GPUData {
  pub texture: vk::Image,
  pub texture_image_view: vk::ImageView,

  pub vertex_buffer: vk::Buffer,
  pub index_buffer: vk::Buffer,

  memories: Vec<MemoryWithType>,
}

#[must_use]
#[derive(Debug)]
pub struct PendingDataInitialization {
  command_buffer_submit: PendingInitialization,
  staging_buffers: SingleUseStagingBuffers<2>,
}

impl PendingDataInitialization {
  // should not fail
  pub unsafe fn wait_and_self_destroy(&self, device: &ash::Device) -> Result<(), QueueSubmitError> {
    self.command_buffer_submit.wait_and_self_destroy(device)?;
    self.staging_buffers.destroy_self(device);
    Ok(())
  }
}

impl DeviceManuallyDestroyed for PendingDataInitialization {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    log::warn!("Aborting and destroying PendingDataInitialization");
    if let Err(err) = self.wait_and_self_destroy(device) {
      log::error!("PendingDataInitialization failed to destroy self: {}", err);
    }
  }
}

fn create_and_copy_from_staging_buffers(
  device: &Device,
  physical_device: &PhysicalDevice,
  queues: &Queues,
  vertex_buffer: vk::Buffer,
  index_buffer: vk::Buffer,
  texture: vk::Image,
  texture_data: (u32, u32, Vec<u8>),
) -> Result<PendingDataInitialization, DeviceMemoryInitializationError> {
  let graphics_pool = command_pools::initialization::InitCommandBufferPool::new(
    device,
    physical_device.queue_families.get_graphics_index(),
  )?;

  unsafe {
    let staging_buffers = allocator::create_single_use_staging_buffers(
      device,
      physical_device,
      [
        (VERTICES.as_ptr() as *const u8, VERTICES_SIZE),
        (QUAD_INDICES.as_ptr() as *const u8, QUAD_INDICES_SIZE),
      ],
      #[cfg(feature = "log_alloc")]
      "DEVICE LOCAL OBJECTS",
    )
    .on_err(|_| graphics_pool.destroy_self(device))?;

    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[0],
      vertex_buffer,
      VERTICES_SIZE,
    );
    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[1],
      index_buffer,
      QUAD_INDICES_SIZE,
    );

    let submit = graphics_pool
      .end_and_submit(device, queues.graphics)
      .on_err(|(pool, _err)| destroy!(device => &staging_buffers, pool))
      .map_err(|(_, err)| err)?;

    Ok(PendingDataInitialization {
      command_buffer_submit: submit,
      staging_buffers,
    })
  }
}

impl GPUData {
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    texture_extent: vk::Extent2D,
    texture_format: vk::Format,
    queues: &Queues,
  ) -> Result<(Self, PendingDataInitialization), DeviceMemoryInitializationError> {
    let texture = create_image(
      device,
      texture_format,
      texture_extent.width,
      texture_extent.height,
      TEXTURE_USAGES,
    )?;
    let vertex_buffer = create_buffer(
      device,
      VERTICES_SIZE,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )
    .on_err(|_| unsafe { texture.destroy_self(device) })?;
    let index_buffer = create_buffer(
      device,
      QUAD_INDICES_SIZE,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )
    .on_err(|_| unsafe { destroy!(device => &vertex_buffer, &texture) })?;

    let device_alloc = allocator::allocate_and_bind_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        vk::MemoryPropertyFlags::empty(),
      ],
      [&vertex_buffer, &index_buffer, &texture],
      0.5,
      #[cfg(feature = "log_alloc")]
      Some(["Vertex buffer", "Index buffer", "Target image"]),
      #[cfg(feature = "log_alloc")]
      "DEVICE LOCAL OBJECTS",
    )
    .on_err(|_| unsafe { destroy!(device => &index_buffer, &vertex_buffer, &texture) })?;

    let pending_device_init = create_and_copy_from_staging_buffers(
      device,
      physical_device,
      queues,
      vertex_buffer,
      index_buffer,
    )
    .on_err(|_| unsafe {
      destroy!(device => &index_buffer, &vertex_buffer, &texture, &device_alloc)
    })?;

    // todo
    let (image_width, image_height, image_bytes) = read_texture_bytes_as_rgba8()?;

    const EXPECTED_MAX_MEM_COUNT: usize = 3;
    let mut memories = Vec::with_capacity(EXPECTED_MAX_MEM_COUNT);
    memories.extend_from_slice(device_alloc.get_memories());
    memories.shrink_to_fit();

    debug_assert!(
      memories.len() <= EXPECTED_MAX_MEM_COUNT,
      "Allocating more than expected"
    );
    log::info!("Allocated memory count: {}", memories.len());

    let texture_image_view = create_image_view(device, texture, texture_format)
    .on_err(|_| unsafe {destroy!(device => &texture, &pending_initialization, &index_buffer, &vertex_buffer, memories.as_slice()) })?;

    Ok((
      Self {
        texture,
        texture_image_view,
        vertex_buffer,
        index_buffer,
        memories,
      },
      pending_initialization,
    ))
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.texture.destroy_self(device);

    self.vertex_buffer.destroy_self(device);
    self.index_buffer.destroy_self(device);

    self.memories.destroy_self(device);
  }
}
