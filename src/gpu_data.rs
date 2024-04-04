use std::{mem::size_of, ops::BitOr};

use ash::vk;

use crate::{
  allocator::allocate_and_bind_memory,
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

pub struct TriangleImage {
  pub image: vk::Image,
  pub image_view: vk::ImageView,
  pub framebuffer: vk::Framebuffer,
}

pub struct TriangleModelData {
  pub vertex_buffer: vk::Buffer,
  pub index_buffer: vk::Buffer,
}

pub struct FinalBuffer {
  pub buffer: vk::Buffer,
  size: u64,
}

pub struct GPUData {
  pub triangle_image: TriangleImage,
  triangle_image_memory: vk::DeviceMemory,
  pub triangle_model: TriangleModelData,
  triangle_model_memory: vk::DeviceMemory,
  pub final_buffer: FinalBuffer,
  final_buffer_memory: vk::DeviceMemory,
}

impl TriangleImage {
  pub fn new(
    device: &ash::Device,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
  ) -> Result<Self, OutOfMemoryError> {
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

    let (triangle_image_memory, triangle_model_memory) =
      Self::allocate_device_memory(device, physical_device, &triangle_image, &triangle_model)
        .on_err(|_| unsafe {
          destroy!(device => &final_buffer, &triangle_model, &triangle_image)
        })?;
    let final_buffer_memory = Self::allocate_host_memory(device, physical_device, &final_buffer)
      .on_err(|_| unsafe {
        triangle_image_memory.destroy_self(device);
        if triangle_model_memory != triangle_image_memory {
          triangle_image_memory.destroy_self(device);
        }
        destroy!(device => &final_buffer, &triangle_model, &triangle_image);
      })?;

    Ok(Self {
      triangle_image,
      triangle_image_memory,
      triangle_model,
      triangle_model_memory,
      final_buffer,
      final_buffer_memory,
    })
  }

  fn allocate_device_memory(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    triangle_image: &TriangleImage,
    triangle_model: &TriangleModelData,
  ) -> Result<(vk::DeviceMemory, vk::DeviceMemory), AllocationError> {
    let triangle_memory_requirements =
      unsafe { device.get_image_memory_requirements(triangle_image.image) };
    let vertex_memory_requirements =
      unsafe { device.get_buffer_memory_requirements(triangle_model.vertex_buffer) };
    let index_memory_requirements =
      unsafe { device.get_buffer_memory_requirements(triangle_model.index_buffer) };

    log::debug!("Allocating device memory for all objects");
    match allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &[triangle_model.vertex_buffer, triangle_model.index_buffer],
      &[vertex_memory_requirements, index_memory_requirements],
      &[triangle_image.image],
      &[triangle_memory_requirements],
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

    let triangle_memory = match allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &[],
      &[],
      &[triangle_image.image],
      &[triangle_memory_requirements],
    ) {
      Ok(alloc) => {
        log::debug!("Triangle image memory allocated successfully");
        alloc.memory
      }
      Err(_) => {
        let alloc = allocate_and_bind_memory(
          device,
          physical_device,
          vk::MemoryPropertyFlags::empty(),
          &[],
          &[],
          &[triangle_image.image],
          &[triangle_memory_requirements],
        )?;
        log::debug!("Triangle image memory allocated suboptimally");
        alloc.memory
      }
    };

    let buffers_memory = match allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &[triangle_model.vertex_buffer, triangle_model.index_buffer],
      &[vertex_memory_requirements, index_memory_requirements],
      &[],
      &[],
    ) {
      Ok(alloc) => {
        log::debug!("Triangle buffers memory allocated successfully");
        alloc.memory
      }
      Err(_) => {
        let alloc = allocate_and_bind_memory(
          device,
          physical_device,
          vk::MemoryPropertyFlags::empty(),
          &[triangle_model.vertex_buffer, triangle_model.index_buffer],
          &[vertex_memory_requirements, index_memory_requirements],
          &[],
          &[],
        )?;
        log::debug!("Triangle buffers memory allocated suboptimally");
        alloc.memory
      }
    };

    Ok((triangle_memory, buffers_memory))
  }

  fn allocate_host_memory(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    final_buffer: &FinalBuffer,
  ) -> Result<vk::DeviceMemory, AllocationError> {
    let final_buffer_memory_requirements =
      unsafe { device.get_buffer_memory_requirements(final_buffer.buffer) };

    log::debug!("Allocating final buffer memory");
    Ok(
      match allocate_and_bind_memory(
        device,
        physical_device,
        vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_CACHED),
        &[final_buffer.buffer],
        &[final_buffer_memory_requirements],
        &[],
        &[],
      ) {
        Ok(alloc) => {
          log::debug!("Final buffer memory allocated successfully");
          alloc.memory
        }
        Err(_) => {
          let alloc = allocate_and_bind_memory(
            device,
            physical_device,
            vk::MemoryPropertyFlags::HOST_VISIBLE,
            &[final_buffer.buffer],
            &[final_buffer_memory_requirements],
            &[],
            &[],
          )?;
          log::debug!("Final buffer memory allocated suboptimally");
          alloc.memory
        }
      },
    )
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
    let data = std::slice::from_raw_parts(ptr, self.final_buffer.size as usize);

    f(data);

    unsafe {
      device.unmap_memory(self.final_buffer_memory);
    }

    Ok(())
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.triangle_image_memory.destroy_self(device);
    if self.triangle_model_memory != self.triangle_image_memory {
      self.triangle_model_memory.destroy_self(device);
    }
    self.final_buffer_memory.destroy_self(device);

    self.triangle_image.destroy_self(device);
    self.triangle_model.destroy_self(device);
    self.final_buffer.destroy_self(device);
  }
}
