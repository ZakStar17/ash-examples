use std::{marker::PhantomData, mem::size_of, ops::BitOr, ptr};

use ash::vk;

use crate::{
  allocator::{self, initialize_device_buffers, MemoryInitializationError, MemoryWithType},
  command_pools::TransferCommandBufferPool,
  create_objs::{create_buffer, create_image, create_image_view},
  device_destroyable::{destroy, DeviceManuallyDestroyed},
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

impl GPUData {
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    render_pass: vk::RenderPass,
    render_extent: vk::Extent2D,
    output_size: u64,
    queues: &Queues,
    command_pool: &mut TransferCommandBufferPool,
  ) -> Result<Self, MemoryInitializationError> {
    let render_target = create_image(
      device,
      render_extent.width,
      render_extent.height,
      vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::COLOR_ATTACHMENT),
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

    let host_output_buffer =
      create_buffer(device, output_size, vk::BufferUsageFlags::TRANSFER_DST)?;

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
    .on_err(|_| unsafe {
      destroy!(device => &host_output_buffer, &index_buffer, &vertex_buffer, &render_target)
    })?;

    unsafe {
      initialize_device_buffers(
        device,
        physical_device,
        [vertex_buffer, index_buffer],
        [
          (VERTICES.as_ptr() as *const u8, VERTEX_SIZE),
          (INDICES.as_ptr() as *const u8, INDEX_SIZE),
        ],
        queues,
        command_pool,
        "DEVICE LOCAL OBJECTS",
      )?;
    }

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
    .on_err(|_| unsafe {
      destroy!(device => &host_output_buffer, &index_buffer, &vertex_buffer, &render_target, &device_alloc)
    })?;
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

    let r_target_image_view =
      create_image_view(device, render_target).on_err(|_| unsafe {
        destroy!(device => &host_output_buffer, &index_buffer, &vertex_buffer, &render_target, memories.as_slice())
      })?;
    let r_target_framebuffer = create_framebuffer(device, render_pass, r_target_image_view, render_extent).on_err(|_| unsafe {
      destroy!(device => &r_target_image_view, &host_output_buffer, &index_buffer, &vertex_buffer, &render_target, memories.as_slice())
    })?;

    Ok(Self {
      render_target,
      r_target_framebuffer,
      r_target_image_view,
      vertex_buffer,
      index_buffer,
      host_output_buffer,
      memories,
      host_output_buffer_memory_index,
      host_output_buffer_memory_offset,
    })
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
        memory: memory,
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
