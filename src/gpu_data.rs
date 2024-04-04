use std::{mem::size_of, ops::BitOr};

use ash::vk;

use crate::{
  create_objs::{create_buffer, create_image, create_image_view},
  destroy,
  device::PhysicalDevice,
  device_destroyable::DeviceManuallyDestroyed,
  errors::{AllocationError, OutOfMemoryError},
  render_pass::create_framebuffer,
  utility::OnErr,
  vertices::Vertex,
  INDICES, VERTICES,
};

struct TriangleImage {
  image: vk::Image,
  image_view: vk::ImageView,
  framebuffer: vk::Framebuffer,
}

struct TriangleModelData {
  vertex_buffer: vk::Buffer,
  index_buffer: vk::Buffer,
}

struct FinalBuffer {
  buffer: vk::Buffer,
  size: u64,
}

struct GPUData {
  triangle_image: TriangleImage,
  triangle_model: TriangleModelData,
  final_buffer: FinalBuffer,
}

impl TriangleImage {
  pub fn new(
    device: &ash::Device,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
  ) -> Result<Self, AllocationError> {
    let image = create_image(
      &device,
      extent.width,
      extent.height,
      vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::COLOR_ATTACHMENT),
    )?;
    let image_view =
      create_image_view(device, image).on_err(|_| unsafe { image.destroy_self(device) })?;
    let framebuffer = create_framebuffer(device, render_pass, image_view, extent)
      .on_err(|_| unsafe { destroy!(device => &image_view, &image) })?;
    Ok(Self {
      image,
      image_view,
      framebuffer,
    })
  }
}

impl DeviceManuallyDestroyed for TriangleImage {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.framebuffer.destroy_self(device);
    self.image_view.destroy_self(device);
    self.image.destroy_self(device);
  }
}

impl TriangleModelData {
  pub fn new(device: &ash::Device) -> Result<Self, OutOfMemoryError> {
    let vertex = create_buffer(
      device,
      (size_of::<Vertex>() * VERTICES.len()) as u64,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )?;
    let index = create_buffer(
      device,
      (size_of::<u16>() * INDICES.len()) as u64,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )
    .on_err(|_| unsafe { vertex.destroy_self(device) })?;
    Ok(Self {
      vertex_buffer: vertex,
      index_buffer: index,
    })
  }
}

impl DeviceManuallyDestroyed for TriangleModelData {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.vertex_buffer.destroy_self(device);
    self.index_buffer.destroy_self(device);
  }
}

impl FinalBuffer {
  pub fn new(device: &ash::Device, size: u64) -> Result<Self, OutOfMemoryError> {
    let buffer = create_buffer(&device, size, vk::BufferUsageFlags::TRANSFER_DST)?;
    Ok(Self { buffer, size })
  }
}

impl DeviceManuallyDestroyed for FinalBuffer {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.buffer.destroy_self(device);
  }
}

impl GPUData {
  pub fn new(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    render_pass: vk::RenderPass,
    image_extent: vk::Extent2D,
    buffer_size: u64,
  ) -> Result<Self, AllocationError> {
    let triangle_image = TriangleImage::new(device, render_pass, image_extent)?;
    let triangle_model =
      TriangleModelData::new(device).on_err(|_| unsafe { triangle_image.destroy_self(device) })?;
    let final_buffer = FinalBuffer::new(device, buffer_size)
      .on_err(|_| unsafe { destroy!(device => &triangle_model, &triangle_image) })?;

    // todo
    log::debug!("Allocating memory for the image that will be cleared");
    let clear_image_memory = match allocate_and_bind_memory(
      &device,
      &physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &[],
      &[],
      &[clear_image],
      &[unsafe { device.get_image_memory_requirements(clear_image) }],
    )
    .or_else(|err| {
      log::warn!("Failed to allocate optimal memory for image:\n{:?}", err);
      allocate_and_bind_memory(
        &device,
        &physical_device,
        vk::MemoryPropertyFlags::empty(),
        &[],
        &[],
        &[clear_image],
        &[unsafe { device.get_image_memory_requirements(clear_image) }],
      )
    }) {
      Ok(alloc) => alloc.memory,
      Err(err) => {
        unsafe {
          clear_image.destroy_self(device);
        }
        return Err(err);
      }
    };

    log::debug!("Allocating memory for the final buffer");
    let final_buffer_memory = match allocate_and_bind_memory(
      &device,
      &physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_CACHED),
      &[final_buffer],
      &[unsafe { device.get_buffer_memory_requirements(final_buffer) }],
      &[],
      &[],
    )
    .or_else(|err| {
      log::warn!(
        "Failed to allocate optimal memory for the final buffer:\n{:?}",
        err
      );
      allocate_and_bind_memory(
        &device,
        &physical_device,
        vk::MemoryPropertyFlags::HOST_VISIBLE,
        &[final_buffer],
        &[unsafe { device.get_buffer_memory_requirements(final_buffer) }],
        &[],
        &[],
      )
    }) {
      Ok(alloc) => alloc.memory,
      Err(err) => {
        unsafe {
          destroy!(device => &clear_image_memory, &clear_image, &final_buffer);
        }
        return Err(err);
      }
    };

    Ok(Self {
      clear_image,
      clear_image_memory,
      final_buffer,
      final_buffer_size: buffer_size,
      final_buffer_memory,
    })
  }

  // map can fail with vk::Result::ERROR_MEMORY_MAP_FAILED
  // in most cases it may be possible to try mapping again a smaller range
  pub unsafe fn get_buffer_data<F: FnOnce(&[u8])>(
    &self,
    device: &ash::Device,
    f: F,
  ) -> Result<(), vk::Result> {
    let ptr = device.map_memory(
      self.final_buffer_memory,
      0,
      // if size is not vk::WHOLE_SIZE, mapping should follow alignments
      vk::WHOLE_SIZE,
      vk::MemoryMapFlags::empty(),
    )? as *const u8;
    let data = std::slice::from_raw_parts(ptr, self.final_buffer_size as usize);

    f(data);

    unsafe {
      device.unmap_memory(self.final_buffer_memory);
    }

    Ok(())
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.clear_image.destroy_self(device);
    self.clear_image_memory.destroy_self(device);
    self.final_buffer.destroy_self(device);
    self.final_buffer_memory.destroy_self(device);
  }
}
