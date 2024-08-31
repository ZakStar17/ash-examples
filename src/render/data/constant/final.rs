use std::ops::BitOr;

use ash::vk;

use crate::{
  render::{
    allocator::allocate_and_bind_memory,
    create_objs::{create_buffer, create_image, create_image_view},
    device_destroyable::{destroy, DeviceManuallyDestroyed},
    errors::AllocationError,
    initialization::device::{Device, PhysicalDevice},
  },
  utility::OnErr,
};

use super::super::{INDEX_SIZE, TEXTURE_FORMAT, TEXTURE_USAGES, VERTEX_SIZE};

const CONSTANT_MEMORY_PRIORITY: f32 = 0.5;

#[derive(Debug)]
pub struct ConstantData {
  pub vertex: vk::Buffer,
  pub index: vk::Buffer,
  pub buffer_memory: vk::DeviceMemory,

  pub texture: vk::Image,
  pub texture_memory: vk::DeviceMemory, // probably equal to buffer_memory
  pub texture_view: vk::ImageView,
}

impl ConstantData {
  pub fn create_and_allocate(
    device: &Device,
    physical_device: &PhysicalDevice,
    image_width: u32,
    image_height: u32,
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
      image_width,
      image_height,
      TEXTURE_USAGES,
    )
    .on_err(|_| unsafe { destroy!(device => &vertex, &index) })?;

    let (buffer_memory, texture_memory) =
      Self::allocate_memory(device, physical_device, vertex, index, texture)
        .on_err(|_| unsafe { destroy!(device => &vertex, &index, &texture) })?;
    let free_device_memory = || unsafe {
      if buffer_memory != texture_memory {
        texture_memory.destroy_self(device);
      }
      buffer_memory.destroy_self(device);
    };

    let texture_view = create_image_view(device, texture, TEXTURE_FORMAT).on_err(|_| unsafe {
      destroy!(device => &vertex, &index, &texture);
      free_device_memory();
    })?;

    Ok(Self {
      vertex,
      index,
      buffer_memory,

      texture,
      texture_memory,
      texture_view,
    })
  }

  fn allocate_memory(
    device: &Device,
    physical_device: &PhysicalDevice,
    vertex: vk::Buffer,
    index: vk::Buffer,
    texture: vk::Image,
  ) -> Result<(vk::DeviceMemory, vk::DeviceMemory), AllocationError> {
    let vertex_memory_requirements = unsafe { device.get_buffer_memory_requirements(vertex) };
    let index_memory_requirements = unsafe { device.get_buffer_memory_requirements(index) };
    let texture_memory_requirements = unsafe { device.get_image_memory_requirements(texture) };

    log::debug!("Allocating device memory for all objects");
    match allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &[vertex, index],
      &[vertex_memory_requirements, index_memory_requirements],
      &[texture],
      &[texture_memory_requirements],
      CONSTANT_MEMORY_PRIORITY,
    ) {
      Ok(alloc) => {
        log::debug!("Allocated full memory block");
        return Ok((alloc.memory, alloc.memory));
      }
      Err(err) => log::warn!(
        "Failed to allocate full memory block, suballocating: {:?}",
        err
      ),
    }

    let buffers_memory = match allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &[vertex, index],
      &[vertex_memory_requirements, index_memory_requirements],
      &[],
      &[],
      CONSTANT_MEMORY_PRIORITY,
    ) {
      Ok(alloc) => {
        log::debug!("Texture buffers memory allocated successfully");
        alloc.memory
      }
      Err(_) => {
        let alloc = allocate_and_bind_memory(
          device,
          physical_device,
          vk::MemoryPropertyFlags::empty(),
          &[vertex, index],
          &[vertex_memory_requirements, index_memory_requirements],
          &[],
          &[],
          CONSTANT_MEMORY_PRIORITY,
        )?;
        log::debug!("Texture buffers memory allocated suboptimally");
        alloc.memory
      }
    };

    let texture_memory = match allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &[],
      &[],
      &[texture],
      &[texture_memory_requirements],
      CONSTANT_MEMORY_PRIORITY,
    ) {
      Ok(alloc) => {
        log::debug!("Texture image memory allocated successfully");
        alloc.memory
      }
      Err(_) => {
        let alloc = allocate_and_bind_memory(
          device,
          physical_device,
          vk::MemoryPropertyFlags::empty(),
          &[],
          &[],
          &[texture],
          &[texture_memory_requirements],
          CONSTANT_MEMORY_PRIORITY,
        )?;
        log::debug!("Texture image memory allocated suboptimally");
        alloc.memory
      }
    };

    Ok((buffers_memory, texture_memory))
  }
}

impl DeviceManuallyDestroyed for ConstantData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.vertex.destroy_self(device);
    self.index.destroy_self(device);

    self.texture_view.destroy_self(device);
    self.texture.destroy_self(device);

    if self.buffer_memory != self.texture_memory {
      self.texture_memory.destroy_self(device);
    }
    self.buffer_memory.destroy_self(device);
  }
}
