use std::ptr;

use ash::vk;

// contains synchronization objects for one frame
pub struct Frame {
  pub image_available: vk::Semaphore,
  pub presentable: vk::Semaphore,
  pub finished: vk::Fence,
}

impl Frame {
  pub fn new(device: &ash::Device) -> Self {
    let semaphore_create_info = vk::SemaphoreCreateInfo {
      s_type: vk::StructureType::SEMAPHORE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::SemaphoreCreateFlags::empty(),
    };

    let create_semaphore = || unsafe {
      device
        .create_semaphore(&semaphore_create_info, None)
        .expect("Failed to create Semaphore")
    };

    let image_available = create_semaphore();
    let presentable = create_semaphore();

    let fence_create_info = vk::FenceCreateInfo {
      s_type: vk::StructureType::FENCE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::FenceCreateFlags::SIGNALED,
    };

    let finished = unsafe {
      device
        .create_fence(&fence_create_info, None)
        .expect("Failed to create Fence Object!")
    };
    Self {
      image_available,
      presentable,
      finished,
    }
  }

  pub fn wait_finished(&self, device: &ash::Device) {
    unsafe {
      device
        .wait_for_fences(&[self.finished], true, u64::MAX)
        .expect("Failed to wait for fences");

      device
        .reset_fences(&[self.finished])
        .expect("Failed to reset fence");
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_semaphore(self.image_available, None);
    device.destroy_semaphore(self.presentable, None);

    device.destroy_fence(self.finished, None);
  }
}
