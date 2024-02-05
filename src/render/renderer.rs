use ash::vk;
use raw_window_handle::RawDisplayHandle;
use winit::{
  dpi::LogicalSize,
  event_loop::EventLoop,
  window::{Window, WindowBuilder},
};

use crate::{INITIAL_WINDOW_HEIGHT, INITIAL_WINDOW_WIDTH, WINDOW_TITLE};

use super::{
  device::{create_logical_device, PhysicalDevice},
  entry::get_entry,
  instance::create_instance,
  validation_layers::DebugUtils,
};

pub struct Renderer {
  pub window: Window,

  physical_device: PhysicalDevice,
  device: ash::Device,
}

impl Renderer {
  pub fn new(entry: &ash::Entry, instance: &ash::Instance, window: Window) -> Self {
    let physical_device = unsafe { PhysicalDevice::select(&instance, &surface_loader) };
    let (device, queues) = create_logical_device(&instance, &physical_device);

    Self {
      _entry: entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
      physical_device,
      device,
    }
  }
}

impl Drop for Renderer {
  fn drop(&mut self) {
    unsafe {
      self.device.destroy_device(None);
      #[cfg(feature = "vulkan_vl")]
      self.debug_utils.destroy_self();
      self.instance.destroy_instance(None);
    }
  }
}
