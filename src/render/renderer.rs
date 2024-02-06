use ash::vk;
use winit::dpi::PhysicalSize;

use super::{
  device::{create_logical_device, PhysicalDevice},
  objects::{Surface, Swapchains},
};

pub struct Renderer {
  physical_device: PhysicalDevice,
  device: ash::Device,

  swapchains: Swapchains
}

impl Renderer {
  pub fn new(instance: &ash::Instance, surface: &Surface, initial_window_size: PhysicalSize<u32>) -> Self {
    let physical_device = unsafe { PhysicalDevice::select(&instance, surface) };
    let (device, queues) = create_logical_device(&instance, &physical_device);

    let swapchains = Swapchains::new(instance, &physical_device, &device, surface, initial_window_size);

    Self {
      physical_device,
      device,

      swapchains
    }
  }

  pub unsafe fn destroy_self(&mut self) {
    self.swapchains.destroy_old(&self.device);
    self.swapchains.destroy_self(&self.device);

    self.device.destroy_device(None);
    
  }
}
