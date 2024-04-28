use std::ops::Deref;

use ash::vk;
use raw_window_handle::{DisplayHandle, WindowHandle};

use crate::render::{device_destroyable::ManuallyDestroyed, errors::OutOfMemoryError};

pub struct Surface {
  inner: vk::SurfaceKHR,
  loader: ash::khr::surface::Instance,
}

impl Deref for Surface {
  type Target = vk::SurfaceKHR;

  fn deref(&self) -> &Self::Target {
    &self.vk_obj
  }
}

#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
  #[error("Out of memory")]
  OutOfMemory(#[source] OutOfMemoryError),
  #[error("Surface is lost")]
  SurfaceIsLost,
}

impl From<vk::Result> for SurfaceError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        SurfaceError::OutOfMemory(OutOfMemoryError::from(value))
      }
      vk::Result::ERROR_SURFACE_LOST_KHR => SurfaceError::SurfaceIsLost,
    }
  }
}

impl Surface {
  pub fn new(
    entry: &ash::Entry,
    instance: &ash::Instance,
    display_handle: DisplayHandle,
    window_handle: WindowHandle,
  ) -> Result<Self, OutOfMemoryError> {
    let loader = ash::khr::surface::Instance::new(&entry, &instance);
    let inner =
      unsafe { ash_window::create_surface(entry, instance, display_handle, window_handle, None) }?;

    Ok(Self { inner, loader })
  }

  pub unsafe fn supports_queue_family(
    &self,
    physical_device: vk::PhysicalDevice,
    family_index: usize,
  ) -> Result<bool, SurfaceError> {
    self
      .loader
      .get_physical_device_surface_support(physical_device, family_index as u32, self.vk_obj)
      .map_err(|err| err.into())
  }

  pub unsafe fn get_formats(
    &self,
    physical_device: vk::PhysicalDevice,
  ) -> Result<Vec<vk::SurfaceFormatKHR>, SurfaceError> {
    self
      .loader
      .get_physical_device_surface_formats(physical_device, self.vk_obj)
      .map_err(|err| err.into())
  }

  pub unsafe fn get_present_modes(
    &self,
    physical_device: vk::PhysicalDevice,
  ) -> Result<Vec<vk::PresentModeKHR>, SurfaceError> {
    self
      .loader
      .get_physical_device_surface_present_modes(physical_device, self.vk_obj)
      .map_err(|err| err.into())
  }

  pub unsafe fn get_capabilities(
    &self,
    physical_device: vk::PhysicalDevice,
  ) -> Result<vk::SurfaceCapabilitiesKHR, SurfaceError> {
    self
      .loader
      .get_physical_device_surface_capabilities(physical_device, self.vk_obj)
      .map_err(|err| err.into())
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
}

impl ManuallyDestroyed for Surface {
  unsafe fn destroy_self(self: &Self) {
    self.loader.destroy_surface(self.vk_obj, None);
  }
}
