mod ferris_model;
mod texture;

use std::{
  marker::PhantomData,
  ptr::{self},
};

use ash::vk;

use crate::{
  render::{
    allocator::allocate_and_bind_memory,
    command_pools::TransferCommandBufferPool,
    create_objs::create_fence,
    device_destroyable::{destroy, DeviceManuallyDestroyed},
    errors::{AllocationError, OutOfMemoryError},
    initialization::device::{PhysicalDevice, Queues},
  },
  utility::OnErr,
};

use self::texture::{LoadedImage, Texture};

use super::{
  command_pools::TemporaryGraphicsCommandPool, create_objs::create_semaphore,
  descriptor_sets::DescriptorPool, errors::InitializationError,
};

pub use self::{ferris_model::FerrisModel, texture::ImageLoadError};

#[derive(Debug)]
pub struct GPUData {
  pub texture: Texture,
  pub ferris: FerrisModel,
}

pub struct StagingMemoryAllocation {
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
    queues: &Queues,
    descriptor_pool: &mut DescriptorPool,
    transfer_pool: &mut TransferCommandBufferPool,
    graphics_pool: &mut TemporaryGraphicsCommandPool,
  ) -> Result<Self, InitializationError> {
    let texture_image = Texture::create_image(device)?;
    let (vertex_final, index_final) = FerrisModel::create_buffers(device)?;

    let destroy_device_objects =
      || unsafe { destroy!(device => &texture_image, &vertex_final, &index_final) };

    let (texture_memory, ferris_memory) = Self::allocate_device_memory(
      device,
      physical_device,
      texture_image.image,
      vertex_final,
      index_final,
    )
    .on_err(|_| destroy_device_objects())?;
    let free_device_memory = || unsafe {
      if texture_memory != ferris_memory {
        texture_memory.destroy_self(device);
      }
      ferris_memory.destroy_self(device);
    };

    let (staging_memory, texture_staging, vertex_staging, index_staging) =
      Self::create_and_populate_staging_objects(device, physical_device, &texture_image).on_err(
        |_| {
          destroy_device_objects();
          free_device_memory();
        },
      )?;
    let destroy_staging = || unsafe {
      destroy!(device => &texture_staging, &vertex_staging, &index_staging);
      staging_memory.memory.destroy_self(device);
    };
    let destroy_all = || {
      destroy_device_objects();
      free_device_memory();
      destroy_staging();
    };

    Self::record_command_buffers_and_dispatch(
      device,
      physical_device,
      queues,
      transfer_pool,
      graphics_pool,
      texture_staging,
      texture_image.image,
      vk::Extent2D {
        width: texture_image.width,
        height: texture_image.height,
      },
      vertex_staging,
      vertex_final,
      FerrisModel::VERTEX_SIZE,
      index_staging,
      index_final,
      FerrisModel::INDEX_SIZE,
    )
    .on_err(|_| destroy_all())?;

    let texture_set = descriptor_pool
      .allocate_sets(device, &[descriptor_pool.texture_layout])
      .unwrap()[0];
    let (texture, texture_write_descriptor_set) = Texture::new(device, texture_image.image, texture_memory, texture_set)
      .on_err(|_| destroy_all())?;

    let ferris = FerrisModel::new(vertex_final, index_final, ferris_memory);

    destroy_staging();

    // update texture set
    unsafe {
      let contextualized_write_descriptor_set = texture_write_descriptor_set.contextualize();
      device.update_descriptor_sets(&[contextualized_write_descriptor_set], &[]);
    }

    Ok(Self { texture, ferris })
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

      Texture::populate_staging_buffer(mem_ptr, &staging_alloc, &texture_image.bytes);
      FerrisModel::populate_staging_buffers(mem_ptr, &staging_alloc);
    }

    Ok((staging_alloc, texture_buffer, vertex_buffer, index_buffer))
  }

  fn record_command_buffers_and_dispatch(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    queues: &Queues,
    transfer_pool: &mut TransferCommandBufferPool,
    graphics_pool: &mut TemporaryGraphicsCommandPool,

    texture_staging: vk::Buffer,
    texture_final: vk::Image,
    texture_dimensions: vk::Extent2D,
    vertex_staging: vk::Buffer,
    vertex_final: vk::Buffer,
    vertex_size: u64,
    index_staging: vk::Buffer,
    index_final: vk::Buffer,
    index_size: u64,
  ) -> Result<(), OutOfMemoryError> {
    let vertex_region = vk::BufferCopy2::default().size(vertex_size);
    let index_region = vk::BufferCopy2::default().size(index_size);
    unsafe {
      transfer_pool.reset(device)?;
      transfer_pool.record_copy_buffers_to_buffers(
        device,
        &[
          vk::CopyBufferInfo2 {
            s_type: vk::StructureType::COPY_BUFFER_INFO_2,
            p_next: ptr::null(),
            src_buffer: vertex_staging,
            dst_buffer: vertex_final,
            region_count: 1,
            p_regions: &vertex_region,
            _marker: PhantomData,
          },
          vk::CopyBufferInfo2 {
            s_type: vk::StructureType::COPY_BUFFER_INFO_2,
            p_next: ptr::null(),
            src_buffer: index_staging,
            dst_buffer: index_final,
            region_count: 1,
            p_regions: &index_region,
            _marker: PhantomData,
          },
        ],
      )?;

      transfer_pool.record_load_texture(
        device,
        &physical_device.queue_families,
        texture_staging,
        texture_final,
        texture_dimensions.width,
        texture_dimensions.height,
      )?;

      if physical_device.queue_families.get_graphics_index()
        != physical_device.queue_families.get_transfer_index()
      {
        graphics_pool.reset(device)?;
        graphics_pool.record_acquire_texture(
          device,
          &physical_device.queue_families,
          texture_final,
        )?;
      }
    }

    let copy_buffers_to_buffers = [transfer_pool.copy_buffers_to_buffers];
    let load_texture = [transfer_pool.load_texture];
    let acquire_texture = [graphics_pool.acquire_texture];

    let ferris_submit_info = vk::SubmitInfo::default().command_buffers(&copy_buffers_to_buffers);

    if physical_device.queue_families.get_graphics_index()
      != physical_device.queue_families.get_transfer_index()
    {
      let texture_finished = create_fence(device)?;
      let ferris_finished =
        create_fence(device).on_err(|_| unsafe { texture_finished.destroy_self(device) })?;
      let wait_texture_transfer = create_semaphore(device)
        .on_err(|_| unsafe { destroy!(device => &texture_finished, &ferris_finished) })?;
      let destroy_objects = || unsafe {
        destroy!(device => &texture_finished, &ferris_finished, &wait_texture_transfer);
      };

      let wait_texture_transfer_arr = [wait_texture_transfer];
      let texture_submit_info_a = vk::SubmitInfo::default()
        .command_buffers(&load_texture)
        .signal_semaphores(&wait_texture_transfer_arr);
      let texture_submit_info_b = vk::SubmitInfo::default()
        .command_buffers(&acquire_texture)
        .wait_semaphores(&wait_texture_transfer_arr)
        .wait_dst_stage_mask(&[vk::PipelineStageFlags::TRANSFER]);

      unsafe {
        device.queue_submit(queues.transfer, &[ferris_submit_info], ferris_finished)?;
        device.queue_submit(queues.transfer, &[texture_submit_info_a], vk::Fence::null())?;
        device.queue_submit(queues.graphics, &[texture_submit_info_b], texture_finished)?;

        device.wait_for_fences(&[ferris_finished, texture_finished], true, u64::MAX)?;

        destroy_objects();
      }
    } else {
      let all_finished = create_fence(device)?;

      let texture_submit_info = vk::SubmitInfo::default().command_buffers(&load_texture);

      unsafe {
        device.queue_submit(
          queues.graphics,
          &[ferris_submit_info, texture_submit_info],
          all_finished,
        )?;
        device.wait_for_fences(&[all_finished], true, u64::MAX)?;

        all_finished.destroy_self(device);
      }
    }
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
