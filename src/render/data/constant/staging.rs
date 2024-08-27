use std::ptr::copy_nonoverlapping;

use ash::vk;

use crate::{
  render::{
    allocator::allocate_and_bind_memory,
    create_objs::create_buffer,
    device_destroyable::{destroy, DeviceManuallyDestroyed},
    errors::AllocationError,
    initialization::device::{Device, PhysicalDevice},
    render_object::{QUAD_INDICES, QUAD_INDICES_SIZE, QUAD_VERTICES, QUAD_VERTICES_SIZE},
  },
  utility::OnErr,
};

use super::super::{INDEX_SIZE, VERTEX_SIZE};

const STAGING_MEMORY_PRIORITY: f32 = 0.3;

#[derive(Debug)]
pub struct StagingData {
  pub vertex: vk::Buffer,
  pub index: vk::Buffer,
  pub texture: vk::Buffer,
  pub alloc: StagingMemoryAllocation,
}

#[derive(Debug)]
pub struct StagingMemoryAllocation {
  pub memory: vk::DeviceMemory,
  pub texture_offset: u64,
  pub vertex_offset: u64,
  pub index_offset: u64,
}

impl StagingData {
  pub fn create_and_allocate(
    device: &Device,
    physical_device: &PhysicalDevice,
    image_size: u64,
  ) -> Result<Self, AllocationError> {
    let vertex = create_buffer(device, VERTEX_SIZE, vk::BufferUsageFlags::TRANSFER_SRC)?;
    let index = create_buffer(device, INDEX_SIZE, vk::BufferUsageFlags::TRANSFER_SRC)
      .on_err(|_| unsafe { destroy!(device => &vertex) })?;

    let texture = create_buffer(device, image_size, vk::BufferUsageFlags::TRANSFER_SRC)
      .on_err(|_| unsafe { destroy!(device => &vertex, &index) })?;

    let alloc = Self::allocate_memory(device, physical_device, vertex, index, texture)
      .on_err(|_| unsafe { destroy!(device => &vertex, &index, &texture) })?;

    Ok(Self {
      vertex,
      index,
      texture,
      alloc,
    })
  }

  pub unsafe fn populate(
    &self,
    device: &ash::Device,
    texture_bytes: &[u8],
  ) -> Result<(), AllocationError> {
    let mem_ptr = device.map_memory(
      self.alloc.memory,
      0,
      vk::WHOLE_SIZE,
      vk::MemoryMapFlags::empty(),
    )? as *mut u8;

    let vertices = QUAD_VERTICES;
    let indices = QUAD_INDICES;
    copy_nonoverlapping(
      vertices.as_ptr() as *const u8,
      mem_ptr.byte_add(self.alloc.vertex_offset as usize),
      QUAD_VERTICES_SIZE,
    );
    copy_nonoverlapping(
      indices.as_ptr() as *const u8,
      mem_ptr.byte_add(self.alloc.index_offset as usize),
      QUAD_INDICES_SIZE,
    );
    copy_nonoverlapping(
      texture_bytes.as_ptr(),
      mem_ptr.byte_add(self.alloc.texture_offset as usize),
      texture_bytes.len(),
    );

    device.unmap_memory(self.alloc.memory);
    Ok(())
  }

  // this function allocates everything in a big block
  // a more concrete way of doing this would be (in a case which a big allocation isn't possible)
  //    to allocate, dispatch and free each object separately to not use much memory
  fn allocate_memory(
    device: &Device,
    physical_device: &PhysicalDevice,
    vertex: vk::Buffer,
    index: vk::Buffer,
    texture: vk::Buffer,
  ) -> Result<StagingMemoryAllocation, AllocationError> {
    let vertex_memory_requirements = unsafe { device.get_buffer_memory_requirements(vertex) };
    let index_memory_requirements = unsafe { device.get_buffer_memory_requirements(index) };
    let texture_memory_requirements = unsafe { device.get_buffer_memory_requirements(texture) };

    log::debug!("Allocating staging memory");
    let allocation = allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE,
      &[vertex, index, texture],
      &[
        vertex_memory_requirements,
        index_memory_requirements,
        texture_memory_requirements,
      ],
      &[],
      &[],
      STAGING_MEMORY_PRIORITY,
    )?;

    let mut offsets_iter = allocation.offsets.buffer_offsets().iter();
    let vertex_offset = *offsets_iter.next().unwrap();
    let index_offset = *offsets_iter.next().unwrap();
    let texture_offset = *offsets_iter.next().unwrap();

    Ok(StagingMemoryAllocation {
      memory: allocation.memory,
      vertex_offset,
      index_offset,
      texture_offset,
    })
  }
}

impl DeviceManuallyDestroyed for StagingData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.vertex.destroy_self(device);
    self.index.destroy_self(device);
    self.texture.destroy_self(device);

    self.alloc.memory.destroy_self(device);
  }
}
