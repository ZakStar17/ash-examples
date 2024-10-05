use std::{
  marker::PhantomData,
  ops::BitOr,
  ptr::{self, copy_nonoverlapping},
};

use ash::vk;

use crate::{
  command_pools::TransferCommandBufferPool,
  create_objs::{create_buffer, create_fence},
  device_destroyable::{fill_destroyable_array_from_iter_using_default, DeviceManuallyDestroyed},
  errors::OutOfMemoryError,
  initialization::device::{Device, PhysicalDevice, Queues},
  utility::OnErr,
};

use super::{AllocationError, MemoryBound, MemoryWithType};

#[derive(Debug, thiserror::Error)]
pub enum MemoryInitializationError {
  #[error("Failed to allocate memory for staging buffers:\n{}", {1})]
  AllocationError(#[from] AllocationError),
  #[error("Object to memory type assignment did not succeed")]
  MemoryMapFailed,
  #[error("Generic out of memory error not caused by a failed allocation ({})", {1})]
  GenericOutOfMemory(#[from] OutOfMemoryError),
}

impl From<vk::Result> for MemoryInitializationError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        Self::GenericOutOfMemory(OutOfMemoryError::from(value))
      }
      vk::Result::ERROR_MEMORY_MAP_FAILED => Self::MemoryMapFailed,
      _ => panic!("Unhandled vk::Result when converting to MemoryInitializationError"),
    }
  }
}

// copy data to buffers
// buffers must already be bounded to valid memory
pub unsafe fn initialize_device_buffers<const S: usize>(
  device: &Device,
  physical_device: &PhysicalDevice,
  buffers: [vk::Buffer; S],
  data: [(*const u8, u64); S],
  queues: &Queues,
  command_pool: &mut TransferCommandBufferPool,
  #[cfg(feature = "log_alloc")] allocation_name: &str,
) -> Result<(), MemoryInitializationError> {
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

  command_pool
    .start_temp_buffer_initialization_recording(device)
    .on_err(|_| destroy_created_objs())?;
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
    command_pool.record_copy_buffers_temp_buffer_initialization(device, &cp_info);
  }
  command_pool
    .finish_temp_buffer_initialization_recording(device)
    .on_err(|_| destroy_created_objs())?;

  let fence = create_fence(device).on_err(|_| destroy_created_objs())?;
  let destroy_created_objs2 = || unsafe {
    destroy_created_objs();
    fence.destroy_self(device);
  };
  let submit_info = vk::SubmitInfo {
    s_type: vk::StructureType::SUBMIT_INFO,
    p_next: ptr::null(),
    wait_semaphore_count: 0,
    p_wait_semaphores: ptr::null(),
    p_wait_dst_stage_mask: ptr::null(),
    command_buffer_count: 1,
    p_command_buffers: &command_pool.temp_buffer_initialization,
    signal_semaphore_count: 0,
    p_signal_semaphores: ptr::null(),
    _marker: PhantomData,
  };
  unsafe {
    device
      .queue_submit(queues.transfer, &[submit_info], fence)
      .on_err(|_| destroy_created_objs2())?;
    device
      .wait_for_fences(&[fence], true, u64::MAX)
      .on_err(|_| destroy_created_objs2())?;
  }

  destroy_created_objs2();

  Ok(())
}
