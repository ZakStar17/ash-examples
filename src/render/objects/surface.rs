use std::ops::Deref;

use ash::vk;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

pub struct Surface {
  vk_obj: vk::SurfaceKHR,
  loader: ash::extensions::khr::Surface,
}

impl Deref for Surface {
  type Target = vk::SurfaceKHR;

  fn deref(&self) -> &Self::Target {
    &self.vk_obj
  }
}

impl Surface {
  pub fn new(
    entry: &ash::Entry,
    instance: &ash::Instance,
    display_handle: RawDisplayHandle,
    window_handle: RawWindowHandle,
  ) -> Self {
    let loader = ash::extensions::khr::Surface::new(&entry, &instance);
    let vk_obj = unsafe {
      ash_window::create_surface(entry, instance, display_handle, window_handle, None)
        .expect("Failed to create window surface")
    };

    Self { vk_obj, loader }
  }

  pub unsafe fn supports_queue_family(
    &self,
    physical_device: vk::PhysicalDevice,
    family_index: usize,
  ) -> bool {
    self
      .loader
      .get_physical_device_surface_support(physical_device, family_index as u32, self.vk_obj)
      .expect("Failed to query for queue family surface support")
  }

  pub unsafe fn get_formats(
    &self,
    physical_device: vk::PhysicalDevice,
  ) -> Vec<vk::SurfaceFormatKHR> {
    self
      .loader
      .get_physical_device_surface_formats(physical_device, self.vk_obj)
      .expect("Failed to get surface formats")
  }

  pub unsafe fn get_present_modes(
    &self,
    physical_device: vk::PhysicalDevice,
  ) -> Vec<vk::PresentModeKHR> {
    self
      .loader
      .get_physical_device_surface_present_modes(physical_device, self.vk_obj)
      .expect("Failed to get surface present modes")
  }

  pub unsafe fn get_capabilities(
    &self,
    physical_device: vk::PhysicalDevice,
  ) -> vk::SurfaceCapabilitiesKHR {
    self
      .loader
      .get_physical_device_surface_capabilities(physical_device, self.vk_obj)
      .expect("Failed to get surface capabilities")
  }

  pub fn get_extent_from_capabilities(
    capabilities: &vk::SurfaceCapabilitiesKHR,
  ) -> Option<vk::Extent2D> {
    if capabilities.current_extent.width != u32::max_value() {
      Some(capabilities.current_extent)
    } else {
      None
    }
  }

  pub unsafe fn destroy_self(&mut self) {
    self.loader.destroy_surface(self.vk_obj, None);
  }
}
