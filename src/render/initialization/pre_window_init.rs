use raw_window_handle::{HandleError, HasDisplayHandle};
use winit::event_loop::{EventLoop, EventLoopWindowTarget};

use crate::render::{device_destroyable::ManuallyDestroyed, renderer::Renderer, SyncRenderer};
use std::mem;

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

  pub fn start(self, event_loop: &EventLoopWindowTarget<()>) -> SyncRenderer {
    // todo: error handling
    let renderer = Renderer::initialize(self, event_loop).unwrap();
    let sync_renderer = SyncRenderer::new(renderer).unwrap();

    sync_renderer
  }

  // take values out without calling drop
  #[cfg(feature = "vl")]
  pub fn deconstruct(mut self) -> (ash::Entry, ash::Instance, DebugUtils) {
    unsafe {
      // there is probably a way better way of doing this,
      //  but I could't get it to work with ManuallyDrop or without creating uninit copies
      let a = mem::replace(&mut self.entry, mem::MaybeUninit::uninit().assume_init());
      let b = mem::replace(&mut self.instance, mem::MaybeUninit::uninit().assume_init());
      let c = mem::replace(
        &mut self.debug_utils,
        mem::MaybeUninit::uninit().assume_init(),
      );
      mem::forget(self);

      (a, b, c)
    }
  }

  #[cfg(not(feature = "vl"))]
  pub fn deconstruct(self) -> (ash::Entry, ash::Instance) {
    unsafe {
      let a = mem::replace(&mut self.entry, mem::MaybeUninit::uninit().assume_init());
      let b = mem::replace(&mut self.instance, mem::MaybeUninit::uninit().assume_init());
      mem::forget(self);

      (a, b)
    }
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
