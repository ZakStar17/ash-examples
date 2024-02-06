use ash::vk;

use super::device::{create_logical_device, PhysicalDevice};

pub struct Renderer {
  physical_device: PhysicalDevice,
  device: ash::Device,
}

impl Renderer {
  pub fn new(
    instance: &ash::Instance,
    surface_loader: &ash::extensions::khr::Surface,
    surface: vk::SurfaceKHR,
  ) -> Self {
    let physical_device = unsafe { PhysicalDevice::select(&instance, &surface_loader, surface) };
    let (device, queues) = create_logical_device(&instance, &physical_device);

    Self {
      physical_device,
      device,
    }
  }
}

impl Drop for Renderer {
  fn drop(&mut self) {
    println!("Drop redenrer");
    unsafe {
      self.device.destroy_device(None);
    }
  }
}
