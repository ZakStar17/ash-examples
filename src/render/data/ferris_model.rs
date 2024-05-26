use std::{mem::size_of, ops::BitOr, ptr::copy_nonoverlapping};

use ash::vk;

use crate::{
  render::{
    create_objs::create_buffer,
    device_destroyable::{destroy, DeviceManuallyDestroyed},
    errors::OutOfMemoryError,
    render_object::{QUAD_INDICES, QUAD_INDICES_SIZE, QUAD_VERTICES, QUAD_VERTICES_SIZE},
    vertices::Vertex,
  },
  utility::OnErr,
};

use super::StagingMemoryAllocation;

#[derive(Debug)]
pub struct FerrisModel {
  pub vertex: vk::Buffer,
  pub index: vk::Buffer,
  pub memory: vk::DeviceMemory, // not owned
}

impl FerrisModel {
  pub const VERTEX_SIZE: u64 = QUAD_VERTICES_SIZE as u64;
  pub const INDEX_SIZE: u64 = QUAD_INDICES_SIZE as u64;
  pub const INDEX_COUNT: u32 = QUAD_INDICES.len() as u32;

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

  pub unsafe fn populate_staging_buffers(mem_ptr: *mut u8, alloc: &StagingMemoryAllocation) {
    let vertices = QUAD_VERTICES;
    let indices = QUAD_INDICES;
    copy_nonoverlapping(
      vertices.as_ptr() as *const u8,
      mem_ptr.byte_add(alloc.vertex_offset as usize),
      QUAD_VERTICES_SIZE,
    );
    copy_nonoverlapping(
      indices.as_ptr() as *const u8,
      mem_ptr.byte_add(alloc.index_offset as usize),
      QUAD_INDICES_SIZE,
    );
  }
}

impl DeviceManuallyDestroyed for FerrisModel {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.vertex.destroy_self(device);
    self.index.destroy_self(device);
  }
}
