use std::ops::BitOr;

use ash::vk;

use crate::{
  render::{
    allocator::{allocate_and_bind_memory, AllocationError, MemoryWithType},
    command_pools::TransferCommandBufferPool,
    create_objs::{create_buffer, create_image, create_image_view},
    device_destroyable::{destroy, DeviceManuallyDestroyed},
    initialization::device::{Device, PhysicalDevice, Queues},
  },
  utility::OnErr,
};

use super::{INDEX_SIZE, TEXTURE_FORMAT, TEXTURE_USAGES, VERTEX_SIZE};

const CONSTANT_MEMORY_PRIORITY: f32 = 0.5;

#[derive(Debug)]
pub struct ConstantData {
  pub vertex: vk::Buffer,
  pub index: vk::Buffer,

  pub texture: vk::Image,
  pub texture_view: vk::ImageView,

  memories: Box<[MemoryWithType]>,
}

impl ConstantData {
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    texture_width: u32,
    texture_height: u32,
    output_size: u64,
    queues: &Queues,
    command_pool: &mut TransferCommandBufferPool,
  ) -> Result<Self, AllocationError> {
    let vertex = create_buffer(
      device,
      VERTEX_SIZE,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )?;
    let index = create_buffer(
      device,
      INDEX_SIZE,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )
    .on_err(|_| unsafe { destroy!(device => &vertex) })?;

    let texture = create_image(
      device,
      TEXTURE_FORMAT,
      texture_width,
      texture_height,
      TEXTURE_USAGES,
    )
    .on_err(|_| unsafe { destroy!(device => &vertex, &index) })?;

    let alloc = allocate_and_bind_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        vk::MemoryPropertyFlags::empty(),
      ],
      [&vertex, &index, &texture],
      CONSTANT_MEMORY_PRIORITY,
      #[cfg(feature = "log_alloc")]
      Some(["Vertex buffer", "Index buffer", "Ferris' texture"]),
      #[cfg(feature = "log_alloc")]
      "CONSTANT DATA",
    )
    .on_err(|_| unsafe { destroy!(device => &vertex, &index, &texture) })?;

    let texture_view = create_image_view(device, texture, TEXTURE_FORMAT)
      .on_err(|_| unsafe { destroy!(device => &vertex, &index, &texture, &alloc) })?;

    let memories = Box::from(alloc.get_memories());
    Ok(Self {
      vertex,
      index,
      texture,
      texture_view,
      memories,
    })
  }
}

impl DeviceManuallyDestroyed for ConstantData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.vertex.destroy_self(device);
    self.index.destroy_self(device);

    self.texture_view.destroy_self(device);
    self.texture.destroy_self(device);

    self.memories.destroy_self(device);
  }
}
