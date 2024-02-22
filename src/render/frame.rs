use std::ptr;

use ash::vk;

use super::common_object_creations::create_signaled_fence;

// contains synchronization objects for one frame
pub struct Frame {
  pub instance_buffer_ready: vk::Semaphore,
  pub compute_finished: vk::Fence,

  pub image_available: vk::Semaphore,
  pub presentable: vk::Semaphore,
  pub graphics_finished: vk::Fence,
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

    let instance_buffer_ready = create_semaphore();
    let compute_finished = create_signaled_fence(device);

    let image_available = create_semaphore();
    let presentable = create_semaphore();
    let graphics_finished = create_signaled_fence(device);

    Self {
      instance_buffer_ready,
      compute_finished,

      image_available,
      presentable,
      graphics_finished,
    }
  }

  pub fn wait_graphics(&self, device: &ash::Device) {
    unsafe {
      device
        .wait_for_fences(&[self.graphics_finished], true, u64::MAX)
        .expect("Failed to wait for fences");
      device
        .reset_fences(&[self.graphics_finished])
        .expect("Failed to reset fence");
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_fence(self.compute_finished, None);
    device.destroy_semaphore(self.instance_buffer_ready, None);

    device.destroy_semaphore(self.image_available, None);
    device.destroy_semaphore(self.presentable, None);
    device.destroy_fence(self.graphics_finished, None);
  }
}
