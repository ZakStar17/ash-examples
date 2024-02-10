use std::{
  fmt::Debug,
  mem::size_of_val,
  ops::BitOr,
  ptr::{self, copy_nonoverlapping},
};

use ash::vk;

use crate::render::{
  objects::{create_image_view, create_semaphore, create_unsignaled_fence},
  vertex::Vertex,
};

use super::{
  command_pools::{TemporaryGraphicsCommandBufferPool, TransferCommandBufferPool},
  device::{PhysicalDevice, Queues},
};

pub struct PackedAllocation {
  pub memory: vk::DeviceMemory,
  pub memory_size: u64,
  pub memory_type: u32,
  pub offsets: AllocationOffsets,
}

pub struct AllocationOffsets {
  buffers_len: usize,
  offsets: Box<[u64]>,
}

impl AllocationOffsets {
  pub fn buffer_offsets(&self) -> &[u64] {
    &self.offsets[0..self.buffers_len]
  }

  pub fn image_offsets(&self) -> &[u64] {
    &self.offsets[self.buffers_len..]
  }
}

pub enum PackedAllocationError {
  MemoryTypeNotSupported,
  TotalSizeExceedsAllowed(u64),
  TotalSizeExceedsHeapSize(u64),
  VkError(vk::Result),
}

impl Debug for PackedAllocationError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::MemoryTypeNotSupported => f.write_str(
        "No device memory type supports all requested buffers and images with the requested properties"
      ),
      Self::TotalSizeExceedsAllowed(size) => f.write_fmt(
        format_args!("Total allocation size ({}) is bigger than the value allowed by the device", size)
      ),
      Self::TotalSizeExceedsHeapSize(size) => f.write_fmt(
        format_args!("A allocation memory type was found but the total allocation size ({}) exceeds its heap capacity", size)
      ),
      Self::VkError(err) => err.fmt(f)
    }
  }
}

// allocates vk::DeviceMemory and binds buffers and images to it with correct alignments
pub fn allocate_and_bind_memory(
  device: &ash::Device,
  physical_device: &PhysicalDevice,
  required_memory_properties: vk::MemoryPropertyFlags,
  optional_memory_properties: vk::MemoryPropertyFlags,
  buffers: &[vk::Buffer],
  images: &[vk::Image],
) -> Result<PackedAllocation, PackedAllocationError> {
  let mut req_mem_type_bits = 0;
  let mut total_size = 0;
  let offsets: Box<[u64]> = buffers
    .iter()
    .map(|&buffer| unsafe { device.get_buffer_memory_requirements(buffer) })
    .chain(
      images
        .iter()
        .map(|&image| unsafe { device.get_image_memory_requirements(image) }),
    )
    .map(|mem_requirements| {
      req_mem_type_bits |= mem_requirements.memory_type_bits;

      // align internal offset to follow memory requirements
      let mut offset = total_size;
      let align_error = offset % mem_requirements.alignment;
      if align_error > 0 {
        offset += mem_requirements.alignment - align_error;
      }

      total_size = offset + mem_requirements.size;
      offset
    })
    .collect();

  let offsets = AllocationOffsets {
    offsets,
    buffers_len: buffers.len(),
  };

  // in this case it can be possible to sub allocate
  if total_size >= physical_device.get_max_memory_allocation_size() {
    return Err(PackedAllocationError::TotalSizeExceedsAllowed(total_size));
  }

  let memory_type = physical_device
    .find_optimal_memory_type(
      req_mem_type_bits,
      required_memory_properties,
      optional_memory_properties,
    )
    .or(Err(PackedAllocationError::MemoryTypeNotSupported))?;

  let heap_size = physical_device.get_memory_type_heap(memory_type).size;
  if total_size >= heap_size {
    return Err(PackedAllocationError::TotalSizeExceedsHeapSize(total_size));
  }

  let allocate_info = vk::MemoryAllocateInfo {
    s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
    p_next: ptr::null(),
    allocation_size: total_size,
    memory_type_index: memory_type,
  };
  let memory = unsafe { device.allocate_memory(&allocate_info, None) }
    .map_err(|vk_err| PackedAllocationError::VkError(vk_err))?;

  for (&buffer, &offset) in buffers.iter().zip(offsets.buffer_offsets().iter()) {
    unsafe { device.bind_buffer_memory(buffer, memory, offset) }
      .map_err(|vk_err| PackedAllocationError::VkError(vk_err))?;
  }
  for (&image, &offset) in images.iter().zip(offsets.image_offsets().iter()) {
    unsafe { device.bind_image_memory(image, memory, offset) }
      .map_err(|vk_err| PackedAllocationError::VkError(vk_err))?;
  }

  Ok(PackedAllocation {
    memory,
    memory_size: total_size,
    memory_type,
    offsets,
  })
}

pub struct ConstantAllocatedObjects {
  memory: vk::DeviceMemory,
  pub vertex: vk::Buffer,
  pub index: vk::Buffer,
  pub texture: vk::Image,
  pub texture_view: vk::ImageView,
}

impl ConstantAllocatedObjects {
  pub const TEXTURE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;

  pub fn new(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    queues: &Queues,
    transfer_command_pool: &mut TransferCommandBufferPool,
    graphics_command_pool: &mut TemporaryGraphicsCommandBufferPool,
    vertices: &[Vertex],
    indices: &[u16],
    texture_bytes: &[u8],
    texture_width: u32,
    texture_height: u32,
  ) -> Self {
    let vertex_size = size_of_val(vertices) as u64;
    let index_size = size_of_val(indices) as u64;
    assert!(texture_bytes.len() == texture_height as usize * texture_width as usize * 4);

    // staging buffers
    let vertex_src = create_buffer(device, vertex_size, vk::BufferUsageFlags::TRANSFER_SRC);
    let index_src = create_buffer(device, index_size, vk::BufferUsageFlags::TRANSFER_SRC);
    let texture_src = create_buffer(
      device,
      texture_bytes.len() as u64,
      vk::BufferUsageFlags::TRANSFER_SRC,
    );

    // final buffers and images
    let vertex_dst = create_buffer(
      device,
      vertex_size as u64,
      vk::BufferUsageFlags::TRANSFER_DST.bitor(vk::BufferUsageFlags::VERTEX_BUFFER),
    );
    let index_dst = create_buffer(
      device,
      index_size as u64,
      vk::BufferUsageFlags::TRANSFER_DST.bitor(vk::BufferUsageFlags::INDEX_BUFFER),
    );
    let texture_dst = create_image(
      device,
      texture_width,
      texture_height,
      Self::TEXTURE_FORMAT,
      vk::ImageTiling::OPTIMAL,
      vk::ImageUsageFlags::TRANSFER_DST.bitor(vk::ImageUsageFlags::SAMPLED),
    );

    log::info!("Allocating staging constant buffers");
    let src_allocation = allocate_and_bind_memory(
      device,
      &physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE,
      vk::MemoryPropertyFlags::HOST_CACHED,
      &[vertex_src, index_src, texture_src],
      &[],
    )
    .expect("Failed to allocate staging constant buffers");
    let src_buffer_offsets = src_allocation.offsets.buffer_offsets();
    let vertex_src_offset = src_buffer_offsets[0];
    let index_src_offset = src_buffer_offsets[1];
    let texture_src_offset = src_buffer_offsets[2];

    log::info!("Allocating constant buffers and textures");
    let dst_allocation = allocate_and_bind_memory(
      device,
      &physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      vk::MemoryPropertyFlags::empty(),
      &[vertex_dst, index_dst],
      &[texture_dst],
    )
    .expect("Failed to allocate constant buffers and textures");

    log::info!("Copying data into staging constant buffers");
    unsafe {
      let mem_ptr = device
        .map_memory(
          src_allocation.memory,
          0,
          vk::WHOLE_SIZE,
          vk::MemoryMapFlags::empty(),
        )
        .expect("Failed to map staging constant source memory") as *mut u8;

      copy_nonoverlapping(
        vertices.as_ptr() as *const u8,
        mem_ptr.byte_add(vertex_src_offset as usize) as *mut u8,
        vertex_size as usize,
      );
      copy_nonoverlapping(
        indices.as_ptr() as *const u8,
        mem_ptr.byte_add(index_src_offset as usize) as *mut u8,
        index_size as usize,
      );
      copy_nonoverlapping(
        texture_bytes.as_ptr(),
        mem_ptr.byte_add(texture_src_offset as usize) as *mut u8,
        texture_bytes.len(),
      );

      let mem_type = physical_device.get_memory_type(src_allocation.memory_type);
      if !mem_type
        .property_flags
        .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
      {
        // doesn't need coherent alignment as it is the whole memory
        let range = vk::MappedMemoryRange {
          s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
          p_next: ptr::null(),
          memory: src_allocation.memory,
          offset: 0,
          size: vk::WHOLE_SIZE,
        };
        device
          .flush_mapped_memory_ranges(&[range])
          .expect("Failed to flush host mapped staging constant buffer memory");
      }

      device.unmap_memory(src_allocation.memory);
    }

    unsafe {
      transfer_command_pool.reset(device);
      graphics_command_pool.reset(device);

      Self::record_buffer_copy(device, transfer_command_pool, vertex_src, vertex_dst, vertex_size, index_src, index_dst, index_size);
      Self::record_texture_load_and_transfer(device, physical_device, transfer_command_pool, graphics_command_pool, texture_src, texture_dst, texture_width, texture_height);
    }

    log::info!("Submitting operations to populate constant buffers and images");
    unsafe {
      Self::submit_and_wait_copy_to_final_objects(device, queues, transfer_command_pool, graphics_command_pool);
    }

    // free staging allocations
    unsafe {
      device.destroy_buffer(vertex_src, None);
      device.destroy_buffer(index_src, None);
      device.destroy_buffer(texture_src, None);
      device.free_memory(src_allocation.memory, None);
    }

    let texture_view = create_image_view(device, texture_dst, Self::TEXTURE_FORMAT);

    Self {
      memory: dst_allocation.memory,
      vertex: vertex_dst,
      index: index_dst,
      texture: texture_dst,
      texture_view,
    }
  }

  unsafe fn record_buffer_copy(
    device: &ash::Device,
    transfer_command_pool: &mut TransferCommandBufferPool,
    vertex_src: vk::Buffer,
    vertex_dst: vk::Buffer,
    vertex_size: u64,
    index_src: vk::Buffer,
    index_dst: vk::Buffer,
    index_size: u64,
  ) {
    let vertex_copy_region = vk::BufferCopy2 {
      s_type: vk::StructureType::BUFFER_COPY_2,
      p_next: ptr::null(),
      src_offset: 0,
      dst_offset: 0,
      size: vertex_size as u64,
    };
    let index_copy_region = vk::BufferCopy2 {
      s_type: vk::StructureType::BUFFER_COPY_2,
      p_next: ptr::null(),
      src_offset: 0,
      dst_offset: 0,
      size: index_size as u64,
    };

    let copy_infos = [
      vk::CopyBufferInfo2 {
        s_type: vk::StructureType::COPY_BUFFER_INFO_2,
        p_next: ptr::null(),
        src_buffer: vertex_src,
        dst_buffer: vertex_dst,
        region_count: 1,
        p_regions: &vertex_copy_region,
      },
      vk::CopyBufferInfo2 {
        s_type: vk::StructureType::COPY_BUFFER_INFO_2,
        p_next: ptr::null(),
        src_buffer: index_src,
        dst_buffer: index_dst,
        region_count: 1,
        p_regions: &index_copy_region,
      },
    ];

    transfer_command_pool.record_copy_buffers(device, &copy_infos);
  }

  unsafe fn record_texture_load_and_transfer(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    transfer_command_pool: &mut TransferCommandBufferPool,
    graphics_command_pool: &mut TemporaryGraphicsCommandBufferPool,
    texture_src: vk::Buffer,
    texture_dst: vk::Image,
    texture_width: u32,
    texture_height: u32,
  ) {
    transfer_command_pool.record_load_texture(
      device,
      &physical_device.queue_families,
      texture_src,
      texture_dst,
      texture_width,
      texture_height,
    );
    graphics_command_pool.record_acquire_texture(
      device,
      &physical_device.queue_families,
      texture_dst,
    );
  }

  unsafe fn submit_and_wait_copy_to_final_objects(
    device: &ash::Device,
    queues: &Queues,
    transfer_command_pool: &mut TransferCommandBufferPool,
    graphics_command_pool: &mut TemporaryGraphicsCommandBufferPool,
  ) {
    let buffer_copy_finished = create_unsignaled_fence(device);
    let buffer_copy_submit_info = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 0,
      p_wait_semaphores: ptr::null(),
      p_wait_dst_stage_mask: ptr::null(),
      command_buffer_count: 1,
      p_command_buffers: &transfer_command_pool.copy_buffers,
      signal_semaphore_count: 0,
      p_signal_semaphores: ptr::null(),
    };
    unsafe {
      device
        .queue_submit(
          queues.transfer,
          &[buffer_copy_submit_info],
          buffer_copy_finished,
        )
        .expect("Failed submit to queue");
    }

    let texture_transfer_finished = create_semaphore(device);
    let texture_acquire_finished = create_unsignaled_fence(device);
    let texture_transfer_submit_info = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 0,
      p_wait_semaphores: ptr::null(),
      p_wait_dst_stage_mask: ptr::null(),
      command_buffer_count: 1,
      p_command_buffers: &transfer_command_pool.load_texture,
      signal_semaphore_count: 1,
      p_signal_semaphores: &texture_transfer_finished,
    };
    let wait_for = vk::PipelineStageFlags::TRANSFER;
    let texture_acquire_submit_info = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 1,
      p_wait_semaphores: &texture_transfer_finished,
      p_wait_dst_stage_mask: &wait_for,
      command_buffer_count: 1,
      p_command_buffers: &graphics_command_pool.acquire_texture,
      signal_semaphore_count: 0,
      p_signal_semaphores: ptr::null(),
    };
    unsafe {
      device
        .queue_submit(
          queues.transfer,
          &[texture_transfer_submit_info],
          vk::Fence::null(),
        )
        .expect("Failed submit to queue");
      device
        .queue_submit(
          queues.graphics,
          &[texture_acquire_submit_info],
          texture_acquire_finished,
        )
        .expect("Failed submit to queue");
    }

    unsafe {
      device
        .wait_for_fences(
          &[buffer_copy_finished, texture_acquire_finished],
          true,
          u64::MAX,
        )
        .unwrap();
    }

    unsafe {
      device.destroy_fence(buffer_copy_finished, None);

      device.destroy_semaphore(texture_transfer_finished, None);
      device.destroy_fence(texture_acquire_finished, None);
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_buffer(self.vertex, None);
    device.destroy_buffer(self.index, None);

    device.destroy_image_view(self.texture_view, None);
    device.destroy_image(self.texture, None);

    device.free_memory(self.memory, None);
  }
}

pub fn create_buffer(device: &ash::Device, size: u64, usage: vk::BufferUsageFlags) -> vk::Buffer {
  assert!(size > 0);
  let create_info = vk::BufferCreateInfo {
    s_type: vk::StructureType::BUFFER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::BufferCreateFlags::empty(),
    size,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(), // ignored when exclusive
  };
  unsafe {
    device
      .create_buffer(&create_info, None)
      .expect("Failed to create buffer")
  }
}

fn create_image(
  device: &ash::Device,
  width: u32,
  height: u32,
  format: vk::Format,
  tiling: vk::ImageTiling,
  usage: vk::ImageUsageFlags,
) -> vk::Image {
  // 1 color layer 2d image
  let create_info = vk::ImageCreateInfo {
    s_type: vk::StructureType::IMAGE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::ImageCreateFlags::empty(),
    image_type: vk::ImageType::TYPE_2D,
    format,
    extent: vk::Extent3D {
      width,
      height,
      depth: 1,
    },
    mip_levels: 1,
    array_layers: 1,
    samples: vk::SampleCountFlags::TYPE_1,
    tiling,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(), // ignored if sharing mode is exclusive
    initial_layout: vk::ImageLayout::UNDEFINED,
  };

  unsafe {
    device
      .create_image(&create_info, None)
      .expect("Failed to create image")
  }
}
