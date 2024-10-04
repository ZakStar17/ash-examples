use std::{
  marker::PhantomData,
  mem::size_of,
  ops::BitOr,
  ptr::{self, copy_nonoverlapping},
};

use ash::vk;

use crate::{
  allocator::{self, allocate_and_bind_memory, AllocationError, MemoryWithType},
  command_pools::TransferCommandBufferPool,
  create_objs::{create_buffer, create_fence, create_image, create_image_view},
  device_destroyable::{destroy, DeviceManuallyDestroyed},
  initialization::device::{Device, PhysicalDevice, Queues},
  render_pass::create_framebuffer,
  utility::OnErr,
  vertices::Vertex,
  INDICES, VERTICES,
};

#[derive(Debug)]
pub struct GPUData {
  pub render_target: vk::Image,
  pub r_target_image_view: vk::ImageView,
  pub r_target_framebuffer: vk::Framebuffer,

  pub vertex_buffer: vk::Buffer,
  pub index_buffer: vk::Buffer,
  pub host_output_buffer: vk::Buffer,
  memories: Vec<MemoryWithType>,
}

impl GPUData {
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    render_pass: vk::RenderPass,
    render_extent: vk::Extent2D,
    output_size: u64,
  ) -> Result<Self, AllocationError> {
    let render_target = create_image(
      device,
      render_extent.width,
      render_extent.height,
      vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::COLOR_ATTACHMENT),
    )?;
    let vertex_buffer = create_buffer(
      device,
      (size_of::<Vertex>() * VERTICES.len()) as u64,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
    )
    .on_err(|_| unsafe { render_target.destroy_self(device) })?;
    let index_buffer = create_buffer(
      device,
      (size_of::<u16>() * INDICES.len()) as u64,
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

    const EXPECTED_MAX_MEM_COUNT: usize = 4;
    let mut memories = Vec::with_capacity(EXPECTED_MAX_MEM_COUNT);
    memories.extend_from_slice(device_alloc.get_memories());
    memories.extend_from_slice(host_output_buffer_alloc.get_memories());

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
    })
  }

  pub fn initialize_memory(
    &mut self,
    device: &Device,
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
      CONSTANT_DATA_PRIORITY,
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

      copy_nonoverlapping(
        VERTICES.as_ptr() as *const u8,
        mem_ptr.byte_add(vertex_offset as usize),
        vertex_size as usize,
      );
      copy_nonoverlapping(
        INDICES.as_ptr() as *const u8,
        mem_ptr.byte_add(index_offset as usize),
        index_size as usize,
      );

      let mem_type = physical_device.get_memory_type(staging_alloc.type_index);
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

  // returns a slice representing buffer contents after all operations have completed
  // map can fail with vk::Result::ERROR_MEMORY_MAP_FAILED
  // in most cases it may be possible to try mapping again a smaller range
  pub unsafe fn map_buffer_after_completion(
    &self,
    device: &ash::Device,
    physical_device: &PhysicalDevice,
  ) -> Result<&[u8], vk::Result> {
    if !physical_device.mem_properties.memory_types[self.final_buffer.memory_type_index as usize]
      .property_flags
      .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
    {
      let range = vk::MappedMemoryRange {
        s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
        p_next: ptr::null(),
        memory: self.final_buffer.memory,
        offset: 0,
        size: vk::WHOLE_SIZE,
        _marker: PhantomData,
      };
      device.invalidate_mapped_memory_ranges(&[range])?;
    }

    let ptr = device.map_memory(
      self.final_buffer.memory,
      0,
      // if size is not vk::WHOLE_SIZE, mapping should follow alignments
      vk::WHOLE_SIZE,
      vk::MemoryMapFlags::empty(),
    )? as *const u8;

    Ok(std::slice::from_raw_parts(
      ptr,
      self.final_buffer.size as usize,
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
