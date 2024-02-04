use std::{
  mem::size_of,
  ops::BitOr,
  ptr::{self, addr_of, copy_nonoverlapping},
};

use ash::vk;
use log::debug;

use crate::{
  command_pools::TransferCommandBufferPool,
  device::{PhysicalDevice, Queues},
  vertex::Vertex,
  INDEX_COUNT, INDICES, VERTEX_COUNT, VERTICES,
};

fn create_buffer(device: &ash::Device, size: u64, usage: vk::BufferUsageFlags) -> vk::Buffer {
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
      .expect("failed to create buffer")
  }
}

struct BuffersAllocation {
  pub memory: vk::DeviceMemory,
  pub memory_size: u64,
  pub memory_type: u32,
  pub offsets: Box<[u64]>,
}

// allocates multiple buffers in one vk::DeviceMemory
fn allocate_buffers(
  device: &ash::Device,
  physical_device: &PhysicalDevice,
  buffers: &[vk::Buffer],
  required_memory_properties: vk::MemoryPropertyFlags,
  optional_memory_properties: vk::MemoryPropertyFlags,
) -> BuffersAllocation {
  let mut req_mem_type_bits = 0;
  let mut alignment = 0;
  let mut req_sizes = Vec::with_capacity(buffers.len());
  for buffer in buffers.iter() {
    let mem_requirements = unsafe { device.get_buffer_memory_requirements(*buffer) };
    req_mem_type_bits |= mem_requirements.memory_type_bits;

    // the specification guarantees that the alignment is a power of 2
    #[cfg(debug_assertions)]
    assert!(
      mem_requirements.alignment > 0
        && (mem_requirements.alignment & (mem_requirements.alignment - 1)) == 0
    );
    if mem_requirements.alignment > alignment {
      alignment = mem_requirements.alignment;
    }

    req_sizes.push(mem_requirements.size);
  }

  let mut total_size = 0;
  let offsets: Box<[u64]> = req_sizes
    .into_iter()
    .map(|size| size + alignment - (size % alignment))
    .map(|aligned_size| {
      let prev = total_size;
      total_size += aligned_size;
      prev
    })
    .collect();

  let memory_type = physical_device
    .find_optimal_memory_type(
      req_mem_type_bits,
      required_memory_properties,
      optional_memory_properties,
    )
    .expect("Failed to find required memory type to allocate buffers");

  let allocate_info = vk::MemoryAllocateInfo {
    s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
    p_next: ptr::null(),
    allocation_size: total_size,
    memory_type_index: memory_type,
  };
  debug!("Allocating buffer memory");
  let buffer_memory = unsafe {
    device
      .allocate_memory(&allocate_info, None)
      .expect("Failed to allocate buffer memory")
  };

  for (buffer, offset) in buffers.iter().zip(offsets.iter()) {
    unsafe {
      device
        .bind_buffer_memory(*buffer, buffer_memory, *offset)
        .expect("Failed to bind buffer to its memory");
    }
  }

  BuffersAllocation {
    memory: buffer_memory,
    memory_size: total_size,
    memory_type,
    offsets,
  }
}

pub struct ConstantBuffers {
  memory: vk::DeviceMemory,
  pub vertex: vk::Buffer,
  pub index: vk::Buffer,
}

impl ConstantBuffers {
  pub fn new(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    queues: &Queues,
    transfer_command_pool: &mut TransferCommandBufferPool,
  ) -> Self {
    let vertices = VERTICES;
    let indices = INDICES;

    let vertex_size = size_of::<[Vertex; VERTEX_COUNT]>();
    let index_size = size_of::<[u16; INDEX_COUNT]>();

    let vertex_buffer_src = create_buffer(
      device,
      vertex_size as u64,
      vk::BufferUsageFlags::TRANSFER_SRC,
    );
    let vertex_buffer_dst = create_buffer(
      device,
      vertex_size as u64,
      vk::BufferUsageFlags::TRANSFER_DST.bitor(vk::BufferUsageFlags::VERTEX_BUFFER),
    );

    let index_buffer_src = create_buffer(
      device,
      index_size as u64,
      vk::BufferUsageFlags::TRANSFER_SRC,
    );
    let index_buffer_dst = create_buffer(
      device,
      index_size as u64,
      vk::BufferUsageFlags::TRANSFER_DST.bitor(vk::BufferUsageFlags::INDEX_BUFFER),
    );

    let host_allocation = allocate_buffers(
      device,
      &physical_device,
      &[vertex_buffer_src, index_buffer_src],
      vk::MemoryPropertyFlags::HOST_VISIBLE,
      vk::MemoryPropertyFlags::HOST_CACHED,
    );
    let vertex_offset = host_allocation.offsets[0];
    let index_offset = host_allocation.offsets[1];

    let local_allocation = allocate_buffers(
      device,
      &physical_device,
      &[vertex_buffer_dst, index_buffer_dst],
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      vk::MemoryPropertyFlags::empty(),
    );

    // copy data into the source buffers (host memory)
    log::info!("Copying constant buffer data into host memory");
    unsafe {
      let mem_ptr = device
        .map_memory(
          host_allocation.memory,
          0,
          host_allocation.memory_size,
          vk::MemoryMapFlags::empty(),
        )
        .expect("Failed to map constant source memory") as *mut u8;

      copy_nonoverlapping(
        addr_of!(vertices) as *const u8,
        mem_ptr.byte_add(vertex_offset as usize) as *mut u8,
        vertex_size,
      );
      copy_nonoverlapping(
        addr_of!(indices) as *const u8,
        mem_ptr.byte_add(index_offset as usize) as *mut u8,
        index_size,
      );

      let mem_type = physical_device.get_memory_type(host_allocation.memory_type);
      if !mem_type
        .property_flags
        .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
      {
        let range = vk::MappedMemoryRange {
          s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
          p_next: ptr::null(),
          memory: host_allocation.memory,
          offset: 0,
          size: host_allocation.memory_size,
        };
        device
          .flush_mapped_memory_ranges(&[range])
          .expect("Failed to flush host mapped constant buffer memory");
      }

      device.unmap_memory(host_allocation.memory);
    }

    // record a copy operation between src and dst buffers
    {
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
          src_buffer: vertex_buffer_src,
          dst_buffer: vertex_buffer_dst,
          region_count: 1,
          p_regions: &vertex_copy_region,
        },
        vk::CopyBufferInfo2 {
          s_type: vk::StructureType::COPY_BUFFER_INFO_2,
          p_next: ptr::null(),
          src_buffer: index_buffer_src,
          dst_buffer: index_buffer_dst,
          region_count: 1,
          p_regions: &index_copy_region,
        },
      ];

      unsafe {
        transfer_command_pool.reset(device);
        transfer_command_pool.record_copy_buffers(device, &copy_infos);
      }
    }

    let finished = {
      let create_info = vk::FenceCreateInfo {
        s_type: vk::StructureType::FENCE_CREATE_INFO,
        p_next: ptr::null(),
        flags: vk::FenceCreateFlags::empty(),
      };
      unsafe {
        device
          .create_fence(&create_info, None)
          .expect("Failed to create fence")
      }
    };

    // submit buffer copy operation
    let submit_info = vk::SubmitInfo {
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
        .queue_submit(queues.transfer, &[submit_info], finished)
        .expect("Failed submit buffer copy operation");
      device.wait_for_fences(&[finished], true, u64::MAX).unwrap();
    }

    // destroy unused objects
    unsafe {
      device.destroy_fence(finished, None);
      for buffer in [vertex_buffer_src, index_buffer_src] {
        device.destroy_buffer(buffer, None);
      }
      device.free_memory(host_allocation.memory, None);
    }

    Self {
      memory: local_allocation.memory,
      vertex: vertex_buffer_dst,
      index: index_buffer_dst,
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_buffer(self.vertex, None);
    device.destroy_buffer(self.index, None);
    device.free_memory(self.memory, None);
  }
}
