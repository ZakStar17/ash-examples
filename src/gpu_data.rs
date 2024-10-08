use std::{marker::PhantomData, mem::size_of, ops::BitOr, ptr};

use ash::vk;

use crate::{
  allocator::{self, DeviceMemoryInitializationError, MemoryWithType, SingleUseStagingBuffers},
  command_pools::{self, initialization::PendingInitialization},
  create_objs::{create_buffer, create_image, create_image_view},
  device_destroyable::{destroy, DeviceManuallyDestroyed},
  errors::QueueSubmitError,
  initialization::device::{Device, PhysicalDevice, Queues},
  render_pass::create_framebuffer,
  utility::OnErr,
  vertices::Vertex,
  INDICES, VERTICES,
};

static VERTEX_SIZE: u64 = (size_of::<Vertex>() * VERTICES.len()) as u64;
static INDEX_SIZE: u64 = (size_of::<u16>() * INDICES.len()) as u64;

#[derive(Debug)]
pub struct GPUData {
  pub render_target: vk::Image,
  pub r_target_image_view: vk::ImageView,
  pub r_target_framebuffer: vk::Framebuffer,

  pub vertex_buffer: vk::Buffer,
  pub index_buffer: vk::Buffer,

  pub host_output_buffer: vk::Buffer,
  host_output_buffer_memory_index: usize,
  host_output_buffer_memory_offset: u64,

  memories: Vec<MemoryWithType>,
}

#[must_use]
#[derive(Debug)]
pub struct PendingDataInitialization {
  command_buffer_submit: PendingInitialization,
  staging_buffers: SingleUseStagingBuffers<2>,
}

impl PendingDataInitialization {
  // should not fail
  pub unsafe fn wait_and_self_destroy(&self, device: &ash::Device) -> Result<(), QueueSubmitError> {
    self.command_buffer_submit.wait_and_self_destroy(device)?;
    self.staging_buffers.destroy_self(device);
    Ok(())
  }
}

impl DeviceManuallyDestroyed for PendingDataInitialization {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    log::warn!("Aborting and destroying PendingDataInitialization");
    if let Err(err) = self.wait_and_self_destroy(device) {
      log::error!("PendingDataInitialization failed to destroy self: {}", err);
    }
  }
}

fn create_and_copy_from_staging_buffers(
  device: &Device,
  physical_device: &PhysicalDevice,
  queues: &Queues,
  vertex_buffer: vk::Buffer,
  index_buffer: vk::Buffer,
) -> Result<PendingDataInitialization, DeviceMemoryInitializationError> {
  let graphics_pool = command_pools::initialization::InitCommandBufferPool::new(
    device,
    physical_device.queue_families.get_graphics_index(),
  )?;

  unsafe {
    let staging_buffers = allocator::create_single_use_staging_buffers(
      device,
      physical_device,
      [
        (VERTICES.as_ptr() as *const u8, VERTEX_SIZE),
        (INDICES.as_ptr() as *const u8, INDEX_SIZE),
      ],
      #[cfg(feature = "log_alloc")]
      "DEVICE LOCAL OBJECTS",
    )
    .on_err(|_| graphics_pool.destroy_self(device))?;

    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[0],
      vertex_buffer,
      VERTEX_SIZE,
    );
    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[1],
      index_buffer,
      INDEX_SIZE,
    );

    let submit = graphics_pool
      .end_and_submit(device, queues.graphics)
      .on_err(|(pool, _err)| destroy!(device => &staging_buffers, pool))
      .map_err(|(_, err)| err)?;

    Ok(PendingDataInitialization {
      command_buffer_submit: submit,
      staging_buffers,
    })
  }
}

impl GPUData {
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    render_pass: vk::RenderPass,
    render_extent: vk::Extent2D,
    output_size: u64,
    queues: &Queues,
  ) -> Result<(Self, PendingDataInitialization), DeviceMemoryInitializationError> {
    let render_target = create_image(
      device,
      render_extent.width,
      render_extent.height,
      vk::ImageUsageFlags::COLOR_ATTACHMENT.bitor(vk::ImageUsageFlags::TRANSFER_SRC),
    )?;
    let vertex_buffer = create_buffer(
      device,
      VERTEX_SIZE,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )
    .on_err(|_| unsafe { render_target.destroy_self(device) })?;
    let index_buffer = create_buffer(
      device,
      INDEX_SIZE,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )
    .on_err(|_| unsafe { destroy!(device => &vertex_buffer, &render_target) })?;

    let device_alloc = allocator::allocate_and_bind_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        vk::MemoryPropertyFlags::empty(),
      ],
      [&vertex_buffer, &index_buffer, &render_target],
      0.5,
      #[cfg(feature = "log_alloc")]
      Some(["Vertex buffer", "Index buffer", "Target image"]),
      #[cfg(feature = "log_alloc")]
      "DEVICE LOCAL OBJECTS",
    )
    .on_err(|_| unsafe { destroy!(device => &index_buffer, &vertex_buffer, &render_target) })?;

    let pending_device_init = create_and_copy_from_staging_buffers(
      device,
      physical_device,
      queues,
      vertex_buffer,
      index_buffer,
    )
    .on_err(|_| unsafe {
      destroy!(device => &index_buffer, &vertex_buffer, &render_target, &device_alloc)
    })?;

    let host_output_buffer = create_buffer(device, output_size, vk::BufferUsageFlags::TRANSFER_DST)
    .on_err(|_| unsafe { destroy!(device => &render_target, &pending_device_init, &index_buffer, &vertex_buffer, &device_alloc) })?;

    let host_output_buffer_alloc = allocator::allocate_and_bind_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_CACHED),
        vk::MemoryPropertyFlags::HOST_VISIBLE,
      ],
      [&host_output_buffer],
      0.5,
      #[cfg(feature = "log_alloc")]
      Some(["Buffer where the final data is read from"]),
      #[cfg(feature = "log_alloc")]
      "OUTPUT BUFFER",
    )
    .on_err(|_| unsafe {destroy!(device => &host_output_buffer, &render_target, &pending_device_init, &index_buffer, &vertex_buffer, &device_alloc) })?;
    let host_output_buffer_memory_offset = host_output_buffer_alloc.obj_to_memory_assignment[0].1;

    const EXPECTED_MAX_MEM_COUNT: usize = 4;
    let mut memories = Vec::with_capacity(EXPECTED_MAX_MEM_COUNT);
    memories.extend_from_slice(device_alloc.get_memories());
    memories.extend_from_slice(host_output_buffer_alloc.get_memories());
    let host_output_buffer_memory_index = memories.len() - 1;
    memories.shrink_to_fit();

    debug_assert!(
      memories.len() <= EXPECTED_MAX_MEM_COUNT,
      "Allocating more than expected"
    );
    log::info!("Allocated memory count: {}", memories.len());

    let r_target_image_view = create_image_view(device, render_target)
    .on_err(|_| unsafe {destroy!(device => &host_output_buffer, &render_target, &pending_device_init, &index_buffer, &vertex_buffer, memories.as_slice()) })?;

    let r_target_framebuffer = create_framebuffer(
      device,
      render_pass,
      r_target_image_view,
      render_extent,
    ).on_err(|_| unsafe {
      destroy!(device => &r_target_image_view, &host_output_buffer, &render_target, &pending_device_init, &index_buffer, &vertex_buffer, memories.as_slice()) })?;

    Ok((
      Self {
        render_target,
        r_target_framebuffer,
        r_target_image_view,
        vertex_buffer,
        index_buffer,
        host_output_buffer,
        memories,
        host_output_buffer_memory_index,
        host_output_buffer_memory_offset,
      },
      pending_device_init,
    ))
  }

  // returns a slice representing buffer contents after all operations have completed
  // map can fail with vk::Result::ERROR_MEMORY_MAP_FAILED
  // in most cases it may be possible to try mapping again a smaller range
  pub unsafe fn map_buffer_after_completion(
    &self,
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    output_size: u64,
  ) -> Result<&[u8], vk::Result> {
    let MemoryWithType { memory, type_index } = self.memories[self.host_output_buffer_memory_index];
    if !physical_device.mem_properties.memory_types[type_index]
      .property_flags
      .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
    {
      let range = vk::MappedMemoryRange {
        s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
        p_next: ptr::null(),
        memory,
        offset: 0,
        size: vk::WHOLE_SIZE,
        _marker: PhantomData,
      };
      device.invalidate_mapped_memory_ranges(&[range])?;
    }

    let ptr = device.map_memory(
      memory,
      0,
      // if size is not vk::WHOLE_SIZE, mapping should follow alignments
      vk::WHOLE_SIZE,
      vk::MemoryMapFlags::empty(),
    )? as *const u8;

    Ok(std::slice::from_raw_parts(
      ptr.byte_add(self.host_output_buffer_memory_offset as usize),
      output_size as usize,
    ))
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.r_target_framebuffer.destroy_self(device);
    self.r_target_image_view.destroy_self(device);
    self.render_target.destroy_self(device);

    self.vertex_buffer.destroy_self(device);
    self.index_buffer.destroy_self(device);

    self.host_output_buffer.destroy_self(device);

    self.memories.destroy_self(device);
  }
}
