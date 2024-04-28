use raw_window_handle::{HandleError, HasDisplayHandle};
use winit::event_loop::EventLoop;

use crate::render::device_destroyable::ManuallyDestroyed;

use super::{DebugUtils, InstanceCreationError};

pub struct RenderInit {
  entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: DebugUtils,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderInitError {
  #[error("Failed to create a Vulkan Instance")]
  InstanceCreationFailed(#[source] InstanceCreationError),

  #[error("Failed to get display handle")]
  DisplayHandle(#[source] HandleError),
}

impl From<InstanceCreationError> for RenderInitError {
  fn from(value: InstanceCreationError) -> Self {
    RenderInitError::InstanceCreationFailed(value)
  }
}

impl RenderInit {
  pub fn new(event_loop: &EventLoop<()>) -> Result<Self, RenderInitError> {
    let entry: ash::Entry = unsafe { super::get_entry() };

    let display_handle = event_loop
      .display_handle()
      .map_err(|err| RenderInitError::DisplayHandle(err))?;

    #[cfg(feature = "vl")]
    let (instance, debug_utils) = super::create_instance(&entry, display_handle)?;
    #[cfg(not(feature = "vl"))]
    let instance = super::create_instance(&entry, event_loop.raw_display_handle())?;

    Ok(Self {
      entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
    })
  }

  #[cfg(feature = "vl")]
  pub fn deconstruct(self) -> (ash::Entry, ash::Instance, DebugUtils) {
    use std::mem;

    mem::forget(self);
    (self.entry, self.instance, self.debug_utils)
  }

  #[cfg(not(feature = "vl"))]
  pub fn deconstruct(self) -> (ash::Entry, ash::Instance) {
    use std::mem;

    mem::forget(self);
    (self.entry, self.instance)
  }
}

impl Drop for RenderInit {
  fn drop(&mut self) {
    unsafe {
      self.instance.destroy_self();
      #[cfg(feature = "vl")]
      self.debug_utils.destroy_self();
    }
  }
}
