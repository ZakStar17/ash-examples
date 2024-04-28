use std::{
  marker::PhantomData,
  mem::size_of,
  ops::BitOr,
  ptr::{self, addr_of, copy_nonoverlapping},
};

use ash::vk;

use crate::{
  allocator::allocate_and_bind_memory,
  command_pools::TransferCommandBufferPool,
  create_objs::{create_buffer, create_fence, create_image, create_image_view},
  destroy,
  device_destroyable::DeviceManuallyDestroyed,
  errors::{AllocationError, OutOfMemoryError},
  initialization::device::{PhysicalDevice, Queues},
  render_pass::create_framebuffer,
  utility::OnErr,
  vertices::Vertex,
  INDICES, VERTICES,
};

pub struct TriangleImage {
  pub image: vk::Image,
  pub memory: vk::DeviceMemory,

  pub image_view: vk::ImageView,
  pub framebuffer: vk::Framebuffer,
}

pub struct TriangleModelData {
  pub vertex: vk::Buffer,
  pub index: vk::Buffer,
  pub memory: vk::DeviceMemory,
}

pub struct FinalBuffer {
  pub buffer: vk::Buffer,
  memory: vk::DeviceMemory,
  size: u64,
}

pub struct GPUData {
  pub triangle_image: TriangleImage,
  pub triangle_model: TriangleModelData,
  pub final_buffer: FinalBuffer,
}

impl GPUData {
  pub fn new(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    render_pass: vk::RenderPass,
    image_extent: vk::Extent2D,
    buffer_size: u64,
  ) -> Result<Self, AllocationError> {
    let triangle_image = create_image(
      &device,
      image_extent.width,
      image_extent.height,
      vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::COLOR_ATTACHMENT),
    )?;
    let vertex_buffer = create_buffer(
      device,
      (size_of::<Vertex>() * VERTICES.len()) as u64,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )
    .on_err(|_| unsafe { triangle_image.destroy_self(device) })?;
    let index_buffer = create_buffer(
      device,
      (size_of::<u16>() * INDICES.len()) as u64,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )
    .on_err(|_| unsafe { destroy!(device => &vertex_buffer, &triangle_image) })?;

    let final_buffer = create_buffer(&device, buffer_size, vk::BufferUsageFlags::TRANSFER_DST)?;

    let destroy_created_objects = || unsafe {
      destroy!(device => &final_buffer, &index_buffer, &vertex_buffer, &triangle_image)
    };

    let (triangle_image_memory, triangle_model_memory) = Self::allocate_device_memory(
      device,
      physical_device,
      triangle_image,
      vertex_buffer,
      index_buffer,
    )
    .on_err(|_| destroy_created_objects())?;
    let final_buffer_memory = Self::allocate_host_memory(device, physical_device, final_buffer)
      .on_err(|_| unsafe {
        destroy_created_objects();

        triangle_image_memory.destroy_self(device);
        if triangle_model_memory != triangle_image_memory {
          triangle_model_memory.destroy_self(device);
        }
      })?;
    let free_memories = || unsafe {
      triangle_image_memory.destroy_self(device);
      if triangle_model_memory != triangle_image_memory {
        triangle_model_memory.destroy_self(device);
      }
      final_buffer_memory.destroy_self(device);
    };

    let triangle_image = TriangleImage::new(
      device,
      triangle_image,
      triangle_image_memory,
      render_pass,
      image_extent,
    )
    .on_err(|_| {
      destroy_created_objects();
      free_memories();
    })?;
    let triangle_model = TriangleModelData::new(vertex_buffer, index_buffer, triangle_model_memory);
    let final_buffer = FinalBuffer::new(final_buffer, final_buffer_memory, buffer_size);

    Ok(Self {
      triangle_image,
      triangle_model,
      final_buffer,
    })
  }

  fn allocate_device_memory(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    triangle_image: vk::Image,
    vertex_buffer: vk::Buffer,
    index_buffer: vk::Buffer,
  ) -> Result<(vk::DeviceMemory, vk::DeviceMemory), AllocationError> {
    let triangle_memory_requirements =
      unsafe { device.get_image_memory_requirements(triangle_image) };
    let vertex_memory_requirements =
      unsafe { device.get_buffer_memory_requirements(vertex_buffer) };
    let index_memory_requirements = unsafe { device.get_buffer_memory_requirements(index_buffer) };

    log::debug!("Allocating device memory for all objects");
    match allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &[vertex_buffer, index_buffer],
      &[vertex_memory_requirements, index_memory_requirements],
      &[triangle_image],
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
      &[triangle_image],
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
          &[triangle_image],
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
      &[vertex_buffer, index_buffer],
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
          &[vertex_buffer, index_buffer],
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
    final_buffer: vk::Buffer,
  ) -> Result<vk::DeviceMemory, AllocationError> {
    let final_buffer_memory_requirements =
      unsafe { device.get_buffer_memory_requirements(final_buffer) };

    log::debug!("Allocating final buffer memory");
    Ok(
      match allocate_and_bind_memory(
        device,
        physical_device,
        vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_CACHED),
        &[final_buffer],
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
            &[final_buffer],
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

  pub fn initialize_memory(
    &mut self,
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    queues: &Queues,
    command_pool: &mut TransferCommandBufferPool,
  ) -> Result<(), AllocationError> {
    let vertex_size = (size_of::<Vertex>() * VERTICES.len()) as u64;
    let index_size = (size_of::<u16>() * INDICES.len()) as u64;

    log::info!("Creating, allocating and populating staging buffers");
    let vertex_src = create_buffer(device, vertex_size, vk::BufferUsageFlags::TRANSFER_SRC)?;
    let index_src = create_buffer(device, index_size, vk::BufferUsageFlags::TRANSFER_SRC)
      .on_err(|_| unsafe { vertex_src.destroy_self(device) })?;
    let destroy_created_objs = || unsafe { destroy!(device => &vertex_src, &index_src) };

    let vertex_src_requirements = unsafe { device.get_buffer_memory_requirements(vertex_src) };
    let index_src_requirements = unsafe { device.get_buffer_memory_requirements(index_src) };

    let staging_alloc = allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE,
      &[vertex_src, index_src],
      &[vertex_src_requirements, index_src_requirements],
      &[],
      &[],
    )
    .on_err(|_| destroy_created_objs())?;
    let vertex_offset = staging_alloc.offsets.buffer_offsets()[0];
    let index_offset = staging_alloc.offsets.buffer_offsets()[1];

    unsafe {
      let mem_ptr = device
        .map_memory(
          staging_alloc.memory,
          0,
          vk::WHOLE_SIZE,
          vk::MemoryMapFlags::empty(),
        )
        .on_err(|_| destroy_created_objs())? as *mut u8;

      let vertices = VERTICES;
      let indices = INDICES;
      copy_nonoverlapping(
        addr_of!(vertices) as *const u8,
        mem_ptr.byte_add(vertex_offset as usize) as *mut u8,
        vertex_size as usize,
      );
      copy_nonoverlapping(
        addr_of!(indices) as *const u8,
        mem_ptr.byte_add(index_offset as usize) as *mut u8,
        index_size as usize,
      );

      let mem_type = physical_device.get_memory_type(staging_alloc.memory_type);
      if !mem_type
        .property_flags
        .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
      {
        let range = vk::MappedMemoryRange {
          s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
          p_next: ptr::null(),
          memory: staging_alloc.memory,
          offset: 0,
          size: vk::WHOLE_SIZE,
          _marker: PhantomData,
        };
        device
          .flush_mapped_memory_ranges(&[range])
          .on_err(|_| destroy_created_objs())?;
      }
    }

    let vertex_region = vk::BufferCopy2 {
      s_type: vk::StructureType::BUFFER_COPY_2,
      p_next: ptr::null(),
      src_offset: 0,
      dst_offset: 0,
      size: vertex_size,
      _marker: PhantomData,
    };
    let index_region = vk::BufferCopy2 {
      size: index_size,
      ..vertex_region
    };
    unsafe {
      command_pool
        .reset(device)
        .on_err(|_| destroy_created_objs())?;
      command_pool.record_copy_buffers_to_buffers(
        device,
        &[
          vk::CopyBufferInfo2 {
            s_type: vk::StructureType::COPY_BUFFER_INFO_2,
            p_next: ptr::null(),
            src_buffer: vertex_src,
            dst_buffer: self.triangle_model.vertex,
            region_count: 1,
            p_regions: &vertex_region,
            _marker: PhantomData,
          },
          vk::CopyBufferInfo2 {
            s_type: vk::StructureType::COPY_BUFFER_INFO_2,
            p_next: ptr::null(),
            src_buffer: index_src,
            dst_buffer: self.triangle_model.index,
            region_count: 1,
            p_regions: &index_region,
            _marker: PhantomData,
          },
        ],
      )?;
    }

    let fence = create_fence(device).on_err(|_| destroy_created_objs())?;
    let destroy_created_objs =
      || unsafe { destroy!(device => &fence, &vertex_src, &index_src, &staging_alloc.memory) };
    let submit_info = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 0,
      p_wait_semaphores: ptr::null(),
      p_wait_dst_stage_mask: ptr::null(),
      command_buffer_count: 1,
      p_command_buffers: &command_pool.copy_buffers_to_buffers,
      signal_semaphore_count: 0,
      p_signal_semaphores: ptr::null(),
      _marker: PhantomData,
    };
    unsafe {
      device
        .queue_submit(queues.transfer, &[submit_info], fence)
        .on_err(|_| destroy_created_objs())?;
      device
        .wait_for_fences(&[fence], true, u64::MAX)
        .on_err(|_| destroy_created_objs())?;
    }

    destroy_created_objs();

    Ok(())
  }

  // map can fail with vk::Result::ERROR_MEMORY_MAP_FAILED
  // in most cases it may be possible to try mapping again a smaller range
  pub unsafe fn get_buffer_data<F: FnOnce(&[u8])>(
    &self,
    device: &ash::Device,
    f: F,
  ) -> Result<(), vk::Result> {
    let ptr = device.map_memory(
      self.final_buffer.memory,
      0,
      // if size is not vk::WHOLE_SIZE, mapping should follow alignments
      vk::WHOLE_SIZE,
      vk::MemoryMapFlags::empty(),
    )? as *const u8;
    let data = std::slice::from_raw_parts(ptr, self.final_buffer.size as usize);

    f(data);

    unsafe {
      device.unmap_memory(self.final_buffer.memory);
    }

    Ok(())
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.triangle_image.destroy_self(device);
    self.triangle_model.destroy_self(device);
    self.final_buffer.destroy_self(device);

    self.triangle_image.memory.destroy_self(device);
    if self.triangle_model.memory != self.triangle_image.memory {
      self.triangle_model.memory.destroy_self(device);
    }
    self.final_buffer.memory.destroy_self(device);
  }
}

impl TriangleImage {
  pub fn new(
    device: &ash::Device,
    image: vk::Image,
    memory: vk::DeviceMemory,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
  ) -> Result<Self, OutOfMemoryError> {
    let image_view =
      create_image_view(device, image).on_err(|_| unsafe { image.destroy_self(device) })?;
    let framebuffer = create_framebuffer(device, render_pass, image_view, extent)
      .on_err(|_| unsafe { image_view.destroy_self(device) })?;
    Ok(Self {
      image,
      memory,
      image_view,
      framebuffer,
    })
  }
}

// memory must be freed manually
impl DeviceManuallyDestroyed for TriangleImage {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.framebuffer.destroy_self(device);
    self.image_view.destroy_self(device);
    self.image.destroy_self(device);
  }
}

impl TriangleModelData {
  pub fn new(vertex: vk::Buffer, index: vk::Buffer, memory: vk::DeviceMemory) -> Self {
    Self {
      vertex,
      index,
      memory,
    }
  }
}

impl DeviceManuallyDestroyed for TriangleModelData {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.vertex.destroy_self(device);
    self.index.destroy_self(device);
  }
}

impl FinalBuffer {
  pub fn new(buffer: vk::Buffer, memory: vk::DeviceMemory, size: u64) -> Self {
    Self {
      buffer,
      memory,
      size,
    }
  }
}

impl DeviceManuallyDestroyed for FinalBuffer {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.buffer.destroy_self(device);
  }
}
