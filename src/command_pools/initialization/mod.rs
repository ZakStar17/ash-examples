use std::{marker::PhantomData, ptr};

use ash::{vk, Device};

use crate::{
  create_objs::create_fence,
  device_destroyable::DeviceManuallyDestroyed,
  errors::QueueSubmitError,
  initialization::device::{QueueFamilies, Queues},
  utility::OnErr,
};

pub struct InitTransferCommandBufferPool {
  pool: vk::CommandPool,
  cb: vk::CommandBuffer,
}

#[must_use]
#[derive(Debug)]
pub struct PendingInitialization {
  pool: vk::CommandPool,
  fence: vk::Fence,
}

impl PendingInitialization {
  pub unsafe fn wait_and_self_destroy(&self, device: &ash::Device) -> Result<(), QueueSubmitError> {
    device
      .wait_for_fences(&[self.fence], true, u64::MAX)?;

    self.fence.destroy_self(device);
    self.pool.destroy_self(device);
    Ok(())
  }
}

impl InitTransferCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Result<Self, vk::Result> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_transfer_index())?;

    let command_buffers = super::allocate_primary_command_buffers(device, pool, 1)?;
    let cb = command_buffers[0];
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    unsafe {
      device.begin_command_buffer(cb, &begin_info)?;
    }

    Ok(Self { pool, cb })
  }

  pub unsafe fn record_copy_buffer_cmd(&self, device: &ash::Device, info: &vk::CopyBufferInfo2) {
    device.cmd_copy_buffer2(self.cb, info);
  }

  pub unsafe fn end_and_submit(
    self,
    device: &Device,
    queues: &Queues,
  ) -> Result<PendingInitialization, QueueSubmitError> {
    let cb: vk::CommandBuffer = self.cb;
    device.end_command_buffer(cb)?;

    let fence = create_fence(device)?;
    let submit_info = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 0,
      p_wait_semaphores: ptr::null(),
      p_wait_dst_stage_mask: ptr::null(),
      command_buffer_count: 1,
      p_command_buffers: &cb,
      signal_semaphore_count: 0,
      p_signal_semaphores: ptr::null(),
      _marker: PhantomData,
    };
    unsafe {
      device
        .queue_submit(queues.transfer, &[submit_info], fence)
        .on_err(|_| fence.destroy_self(device))?;
    }
    Ok(PendingInitialization {
      pool: self.pool,
      fence,
    })
  }
}

// should not be called while buffer is in submission
impl DeviceManuallyDestroyed for InitTransferCommandBufferPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
