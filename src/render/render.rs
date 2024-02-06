use std::mem::ManuallyDrop;

use ash::vk;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::{
  dpi::LogicalSize,
  event_loop::{EventLoop, EventLoopWindowTarget},
  window::{Window, WindowBuilder},
};

use crate::{INITIAL_WINDOW_HEIGHT, INITIAL_WINDOW_WIDTH, WINDOW_TITLE};

#[cfg(feature = "vl")]
use super::validation_layers::DebugUtils;
use super::{entry::get_entry, instance::create_instance, sync_renderer::SyncRenderer};

pub struct Render {
  entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: DebugUtils,

  windowed: Option<ManuallyDrop<WindowedRender>>,
}

impl Render {
  pub fn init(event_loop: &EventLoop<()>) -> Self {
    let entry: ash::Entry = unsafe { get_entry() };

    #[cfg(feature = "vl")]
    let (instance, debug_utils) = create_instance(&entry, event_loop.raw_display_handle());
    #[cfg(not(feature = "vl"))]
    let instance = instance::create_instance(&entry, event_loop);

    Self {
      entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
      windowed: None,
    }
  }

  pub fn start(&mut self, target: &EventLoopWindowTarget<()>) {
    assert!(self.windowed.is_none());

    self.windowed = Some(ManuallyDrop::new(WindowedRender::new(
      target,
      &self.entry,
      &self.instance,
    )));
  }

  pub fn draw(&mut self) {}
}

impl Drop for Render {
  fn drop(&mut self) {
    unsafe {
      if let Some(windowed) = self.windowed.as_mut() {
        ManuallyDrop::drop(windowed);
      }

      self.debug_utils.destroy_self();
      self.instance.destroy_instance(None);
    }
  }
}

fn create_window(target: &EventLoopWindowTarget<()>) -> Window {
  WindowBuilder::new()
    .with_title(WINDOW_TITLE)
    .with_inner_size(LogicalSize::new(
      INITIAL_WINDOW_WIDTH,
      INITIAL_WINDOW_HEIGHT,
    ))
    .build(target)
    .expect("Failed to create window.")
}

struct WindowedRender {
  window: Window,
  surface_loader: ash::extensions::khr::Surface,
  surface: vk::SurfaceKHR,
  renderer: SyncRenderer,
}

impl WindowedRender {
  pub fn new(
    target: &EventLoopWindowTarget<()>,
    entry: &ash::Entry,
    instance: &ash::Instance,
  ) -> Self {
    let window = create_window(target);

    let surface_loader = ash::extensions::khr::Surface::new(&entry, &instance);
    let surface = unsafe {
      ash_window::create_surface(
        entry,
        instance,
        target.raw_display_handle(),
        window.raw_window_handle(),
        None,
      )
      .expect("Failed to create window surface")
    };

    let renderer = SyncRenderer::new(instance, &surface_loader, surface);

    Self {
      window,
      surface_loader,
      surface,
      renderer,
    }
  }
}

impl Drop for WindowedRender {
  fn drop(&mut self) {
    unsafe {
      self.surface_loader.destroy_surface(self.surface, None);
    }
  }
}
