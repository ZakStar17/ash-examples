use std::{mem::size_of, ops::BitOr, ptr::copy_nonoverlapping};

use ash::vk;

use crate::{
  destroy,
  render::{
    create_objs::create_buffer,
    device_destroyable::DeviceManuallyDestroyed,
    errors::OutOfMemoryError,
    render_object::{QUAD_INDICES, QUAD_INDICES_SIZE, QUAD_VERTICES, QUAD_VERTICES_SIZE},
    vertices::Vertex,
  },
  utility::OnErr,
};

use super::StagingMemoryAllocation;

pub struct FerrisModel {
  pub vertex: vk::Buffer,
  pub index: vk::Buffer,
  pub memory: vk::DeviceMemory, // not owned
}

impl FerrisModel {
  pub fn new(vertex: vk::Buffer, index: vk::Buffer, memory: vk::DeviceMemory) -> Self {
    Self {
      vertex,
      index,
      memory,
    }
  }

  pub fn create_buffers(
    device: &ash::Device,
  ) -> Result<(vk::Buffer, vk::Buffer), OutOfMemoryError> {
    let vertex = create_buffer(
      device,
      (size_of::<Vertex>() * QUAD_VERTICES.len()) as u64,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )?;
    let index = create_buffer(
      device,
      (size_of::<u16>() * QUAD_INDICES.len()) as u64,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )
    .on_err(|_| unsafe { destroy!(device => &vertex) })?;

    Ok((vertex, index))
  }

  pub fn create_staging_buffers(
    device: &ash::Device,
  ) -> Result<(vk::Buffer, vk::Buffer), OutOfMemoryError> {
    let vertex = create_buffer(
      device,
      (size_of::<Vertex>() * QUAD_VERTICES.len()) as u64,
      vk::BufferUsageFlags::TRANSFER_SRC,
    )?;
    let index = create_buffer(
      device,
      (size_of::<u16>() * QUAD_INDICES.len()) as u64,
      vk::BufferUsageFlags::TRANSFER_SRC,
    )
    .on_err(|_| unsafe { destroy!(device => &vertex) })?;

    Ok((vertex, index))
  }

  pub unsafe fn populate_staging_buffers(mem_ptr: *mut u8, alloc: StagingMemoryAllocation) {
    let vertices = QUAD_VERTICES;
    let indices = QUAD_INDICES;
    copy_nonoverlapping(
      vertices.as_ptr() as *const u8,
      mem_ptr.byte_add(alloc.vertex_offset as usize) as *mut u8,
      QUAD_VERTICES_SIZE,
    );
    copy_nonoverlapping(
      indices.as_ptr() as *const u8,
      mem_ptr.byte_add(alloc.index_offset as usize) as *mut u8,
      QUAD_INDICES_SIZE,
    );
  }
}

impl DeviceManuallyDestroyed for FerrisModel {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.vertex.destroy_self(device);
    self.index.destroy_self(device);
  }
}
