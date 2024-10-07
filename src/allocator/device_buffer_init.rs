use std::{
  marker::PhantomData,
  ops::BitOr,
  ptr::{self, copy_nonoverlapping},
};

use ash::vk;

use crate::{
  command_pools::initialization::InitTransferCommandBufferPool,
  create_objs::create_buffer,
  device_destroyable::{fill_destroyable_array_from_iter_using_default, DeviceManuallyDestroyed},
  errors::{DeviceIsLost, OutOfMemoryError},
  initialization::device::{Device, PhysicalDevice},
  utility::OnErr,
};

use super::{AllocationError, MemoryBound, MemoryWithType};

#[derive(Debug, thiserror::Error)]
pub enum RecordMemoryInitializationFailedError {
  #[error("Failed to allocate memory for staging buffers:\n{}", {1})]
  AllocationError(#[from] AllocationError),
  #[error("Object to memory type assignment on staging buffers did not succeed")]
  MemoryMapFailed,
  #[error("Generic out of memory error not caused by a failed allocation ({})", {1})]
  GenericOutOfMemory(#[from] OutOfMemoryError),
  #[error(transparent)]
  DeviceIsLost(#[from] DeviceIsLost),
}

impl From<vk::Result> for RecordMemoryInitializationFailedError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        Self::GenericOutOfMemory(OutOfMemoryError::from(value))
      }
      vk::Result::ERROR_MEMORY_MAP_FAILED => Self::MemoryMapFailed,
      vk::Result::ERROR_DEVICE_LOST => Self::DeviceIsLost(DeviceIsLost {}),
      _ => panic!("Unhandled vk::Result when converting to MemoryInitializationError"),
    }
  }
}

// to be destroyed after command buffer finishes
#[must_use]
#[derive(Debug)]
pub struct InitializationStagingBuffers<const S: usize> {
  buffers: [vk::Buffer; S],
  memories: [vk::DeviceMemory; vk::MAX_MEMORY_TYPES],
  memory_count: usize,
}

impl<const S: usize> DeviceManuallyDestroyed for InitializationStagingBuffers<S> {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.buffers.destroy_self(device);
    self.memories[0..self.memory_count].destroy_self(device);
  }
}

// creates staging buffers with data to be copied and records appropriate commands to the
// initialization command buffer
pub unsafe fn record_device_buffer_initialization<const S: usize>(
  device: &Device,
  physical_device: &PhysicalDevice,
  buffers: [vk::Buffer; S],
  data: [(*const u8, u64); S],
  // contains one command buffer, must be already in recording state
  record_pool: &InitTransferCommandBufferPool,
  #[cfg(feature = "log_alloc")] allocation_name: &str,
) -> Result<InitializationStagingBuffers<S>, RecordMemoryInitializationFailedError> {
  assert!(buffers.len() > 0);
  let staging_buffers: [vk::Buffer; S] = fill_destroyable_array_from_iter_using_default!(
    device,
    data
      .iter()
      .map(|&(_, size)| create_buffer(device, size, vk::BufferUsageFlags::TRANSFER_SRC)),
    S
  )?;
  let trait_objs = {
    let mut tmp: [&dyn MemoryBound; S] = [&staging_buffers[0]; S];
    for i in 0..S {
      tmp[i] = &staging_buffers[i];
    }
    tmp
  };

  let staging_alloc = super::allocate_and_bind_memory(
    device,
    physical_device,
    [
      vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_COHERENT),
      vk::MemoryPropertyFlags::HOST_VISIBLE,
    ],
    trait_objs,
    0.5,
    #[cfg(feature = "log_alloc")]
    None,
    #[cfg(feature = "log_alloc")]
    &format!("INITIALIZATION STAGING BUFFERS <{}>", allocation_name),
  )
  .on_err(|_| unsafe {
    staging_buffers.destroy_self(device);
  })?;

  let destroy_created_objs = || unsafe {
    staging_buffers.destroy_self(device);
    staging_alloc.destroy_self(device);
  };

  let mem_ptrs = {
    let mut tmp = [ptr::null_mut(); S];
    for (
      i,
      MemoryWithType {
        memory,
        type_index: _,
      },
    ) in staging_alloc.get_memories().iter().enumerate()
    {
      let mem_ptr = device
        .map_memory(*memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
        .on_err(|_| destroy_created_objs())? as *mut u8;
      tmp[i] = mem_ptr;
    }
    tmp
  };

  for ((mem_i, offset), (ptr, size)) in staging_alloc
    .obj_to_memory_assignment
    .into_iter()
    .zip(data.into_iter())
  {
    copy_nonoverlapping(
      ptr,
      mem_ptrs[mem_i].byte_add(offset as usize),
      size as usize,
    );
  }

  // no explicit flushing: memory flushed implicitly on queue submit

  for i in 0..S {
    let region = vk::BufferCopy2::default().size(data[i].1);
    let cp_info = vk::CopyBufferInfo2 {
      s_type: vk::StructureType::COPY_BUFFER_INFO_2,
      p_next: ptr::null(),
      src_buffer: staging_buffers[i],
      dst_buffer: buffers[i],
      region_count: 1,
      p_regions: &region,
      _marker: PhantomData,
    };
    record_pool.record_copy_buffer_cmd(device, &cp_info);
  }

  let memories = staging_alloc.memories.map(|m| m.memory);
  Ok(InitializationStagingBuffers {
    buffers,
    memories,
    memory_count: staging_alloc.memory_count,
  })
}
