mod ferris_model;
mod texture;

use std::{
  marker::PhantomData,
  mem::size_of,
  ops::BitOr,
  ptr::{self, addr_of, copy_nonoverlapping},
};

use ash::vk;

use crate::{
  destroy,
  render::{
    allocator::allocate_and_bind_memory,
    command_pools::TransferCommandBufferPool,
    create_objs::{create_buffer, create_fence, create_image, create_image_view},
    device_destroyable::DeviceManuallyDestroyed,
    errors::{AllocationError, OutOfMemoryError},
    initialization::device::{PhysicalDevice, Queues},
    render_pass::create_framebuffer,
    vertices::Vertex,
  },
  utility::OnErr,
};

use self::{
  ferris_model::FerrisModel,
  texture::{LoadedImage, Texture},
};

use super::errors::InitializationError;

pub use self::texture::ImageLoadError;

pub struct GPUData {
  pub texture: Texture,
  pub ferris: FerrisModel,
}

struct StagingMemoryAllocation {
  pub memory: vk::DeviceMemory,
  pub memory_type: u32,
  pub texture_offset: u64,
  pub vertex_offset: u64,
  pub index_offset: u64,
}

impl GPUData {
  pub fn new(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    render_pass: vk::RenderPass,
  ) -> Result<Self, InitializationError> {
    let texture_image = Texture::create_image(device)?;
    let (vertex_buffer, index_buffer) = FerrisModel::create_buffers(device)?;

    let destroy_device_objects =
      || unsafe { destroy!(device => &texture_image, &vertex_buffer, &index_buffer) };

    let (texture_memory, ferris_memory) = Self::allocate_device_memory(
      device,
      physical_device,
      texture_image.image,
      vertex_buffer,
      index_buffer,
    )
    .on_err(|_| destroy_device_objects())?;
    let free_device_memory = || unsafe {
      if texture_memory != ferris_memory {
        texture_memory.destroy_self(device);
      }
      ferris_memory.destroy_self(device);
    };

    let (staging_memory, staging_texture_buffer, staging_vertex_buffer, staging_index_buffer) =
      Self::create_and_populate_staging_objects(device, physical_device, &texture_image).on_err(
        |_| {
          destroy_device_objects();
          free_device_memory();
        },
      )?;

    Ok(Self {
      triangle_image,
      triangle_model,
      final_buffer,
    })
  }

  fn allocate_device_memory(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    texture_image: vk::Image,
    vertex_buffer: vk::Buffer,
    index_buffer: vk::Buffer,
  ) -> Result<(vk::DeviceMemory, vk::DeviceMemory), AllocationError> {
    let texture_memory_requirements =
      unsafe { device.get_image_memory_requirements(texture_image) };
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
      &[texture_image],
      &[texture_memory_requirements],
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

    let texture_memory = match allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &[],
      &[],
      &[texture_image],
      &[texture_memory_requirements],
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
          &[texture_image],
          &[texture_memory_requirements],
        )?;
        log::debug!("Texture image memory allocated suboptimally");
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
        log::debug!("Texture buffers memory allocated successfully");
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
        log::debug!("Texture buffers memory allocated suboptimally");
        alloc.memory
      }
    };

    Ok((texture_memory, buffers_memory))
  }

  // this function allocates everything in a big block
  // a more concrete way of doing this would be (in a case which a big allocation isn't possible)
  //    to allocate, dispatch and free each object separately to not use much memory
  fn allocate_staging_memory(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    texture_buffer: vk::Buffer,
    vertex_buffer: vk::Buffer,
    index_buffer: vk::Buffer,
  ) -> Result<StagingMemoryAllocation, AllocationError> {
    let texture_memory_requirements =
      unsafe { device.get_buffer_memory_requirements(texture_buffer) };
    let vertex_memory_requirements =
      unsafe { device.get_buffer_memory_requirements(vertex_buffer) };
    let index_memory_requirements = unsafe { device.get_buffer_memory_requirements(index_buffer) };

    log::debug!("Allocating staging memory");
    let allocation = allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE,
      &[texture_buffer, vertex_buffer, index_buffer],
      &[
        texture_memory_requirements,
        vertex_memory_requirements,
        index_memory_requirements,
      ],
      &[],
      &[],
    )?;
    let mut offsets_iter = allocation.offsets.buffer_offsets().iter();

    Ok(StagingMemoryAllocation {
      memory: allocation.memory,
      memory_type: allocation.memory_type,
      texture_offset: *offsets_iter.next().unwrap(),
      vertex_offset: *offsets_iter.next().unwrap(),
      index_offset: *offsets_iter.next().unwrap(),
    })
  }

  fn create_and_populate_staging_objects(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    texture_image: &LoadedImage,
  ) -> Result<(StagingMemoryAllocation, vk::Buffer, vk::Buffer, vk::Buffer), AllocationError> {
    let texture_buffer = Texture::create_staging_buffer(device, texture_image)?;
    let (vertex_buffer, index_buffer) =
      FerrisModel::create_staging_buffers(device).on_err(|_| unsafe {
        destroy!(&device => &texture_buffer);
      })?;
    let destroy_staging_objects = || unsafe {
      destroy!(&device => &texture_buffer, &vertex_buffer, &index_buffer);
    };

    let staging_alloc = Self::allocate_staging_memory(
      device,
      physical_device,
      texture_buffer,
      vertex_buffer,
      index_buffer,
    )
    .on_err(|_| destroy_staging_objects())?;
    let destroy_and_exit = || unsafe {
      destroy_staging_objects();
      staging_alloc.memory.destroy_self(device);
    };

    unsafe {
      // memory could be mapped into smaller sizes in case of an error
      let mem_ptr = device
        .map_memory(
          staging_alloc.memory,
          0,
          vk::WHOLE_SIZE,
          vk::MemoryMapFlags::empty(),
        )
        .on_err(|_| destroy_and_exit())? as *mut u8;

      Texture::populate_staging_buffer(mem_ptr, staging_alloc, &texture_image.bytes);
      FerrisModel::populate_staging_buffers(mem_ptr, staging_alloc);
    }

    Ok((staging_alloc, texture_buffer, vertex_buffer, index_buffer))
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

  // todo
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
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.texture.destroy_self(device);
    self.ferris.destroy_self(device);

    if self.texture.memory != self.ferris.memory {
      self.texture.memory.destroy_self(device);
    }
    self.ferris.memory.destroy_self(device);
  }
}
