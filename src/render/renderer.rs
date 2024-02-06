use ash::vk;
use winit::dpi::PhysicalSize;

use super::{
  device::{create_logical_device, PhysicalDevice},
  objects::{create_framebuffer, create_render_pass, Surface, Swapchains},
};

pub struct Renderer {
  physical_device: PhysicalDevice,
  device: ash::Device,

  swapchains: Swapchains,
  render_pass: vk::RenderPass,
  framebuffers: Box<[vk::Framebuffer]>,
}

impl Renderer {
  pub fn new(
    instance: &ash::Instance,
    surface: &Surface,
    initial_window_size: PhysicalSize<u32>,
  ) -> Self {
    let physical_device = unsafe { PhysicalDevice::select(&instance, surface) };
    let (device, queues) = create_logical_device(&instance, &physical_device);

    let swapchains = Swapchains::new(
      instance,
      &physical_device,
      &device,
      surface,
      initial_window_size,
    );

    let render_pass = create_render_pass(&device, swapchains.get_format());
    let framebuffers = swapchains
      .get_image_views()
      .iter()
      .map(|&view| create_framebuffer(&device, render_pass, view, swapchains.get_extent()))
      .collect();

    Self {
      physical_device,
      device,

      swapchains,
      render_pass,
      framebuffers,
    }
  }

  pub unsafe fn destroy_self(&mut self) {
    for &framebuffer in self.framebuffers.iter() {
      self.device.destroy_framebuffer(framebuffer, None);
    }
    self.device.destroy_render_pass(self.render_pass, None);

    self.swapchains.destroy_old(&self.device);
    self.swapchains.destroy_self(&self.device);

    self.device.destroy_device(None);
  }
}
