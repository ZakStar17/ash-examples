use ash::vk;

pub trait ManuallyDestroyed {
  unsafe fn destroy_self(self: &Self);
}
pub trait DeviceManuallyDestroyed {
  unsafe fn destroy_self(self: &Self, device: &ash::Device);
}

impl<T: ManuallyDestroyed> DeviceManuallyDestroyed for T {
  unsafe fn destroy_self(self: &Self, _device: &ash::Device) {
    self.destroy_self();
  }
}

#[macro_export(local_inner_macros)]
macro_rules! destroy {
  ($($obj:expr),+) => {
    {
      use crate::render::device_destroyable::ManuallyDestroyed;
      $(ManuallyDestroyed::destroy_self($obj);)+
    }
  };

  ($device:expr => $($obj:expr),+) => {
    {
      use crate::render::device_destroyable::DeviceManuallyDestroyed;
      $(DeviceManuallyDestroyed::destroy_self($obj, $device);)+
    }
  };
}

impl ManuallyDestroyed for ash::Instance {
  unsafe fn destroy_self(self: &Self) {
    self.destroy_instance(None);
  }
}

impl ManuallyDestroyed for ash::Device {
  unsafe fn destroy_self(self: &Self) {
    self.destroy_device(None);
  }
}

impl DeviceManuallyDestroyed for vk::Fence {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_fence(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::Semaphore {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_semaphore(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::Image {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_image(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::Buffer {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_buffer(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::DeviceMemory {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.free_memory(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::RenderPass {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_render_pass(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::PipelineCache {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_pipeline_cache(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::ImageView {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_image_view(*self, None);
  }
}

impl DeviceManuallyDestroyed for vk::Framebuffer {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_framebuffer(*self, None);
  }
}
