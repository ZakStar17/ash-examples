use ash::vk;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::{
  dpi::LogicalSize,
  event_loop::EventLoop,
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

  windowed: Option<WindowedRender>,
}

impl Render {
  pub fn init(event_loop: &EventLoop<()>) -> Self {
    let entry: ash::Entry = unsafe { get_entry() };

    #[cfg(feature = "vl")]
    let (instance, mut debug_utils) = create_instance(&entry, event_loop.raw_display_handle());
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

  // creates the window and starts rendering if haven't already
  pub fn start(&mut self, event_loop: &EventLoop<()>) {
    assert!(self.windowed.is_none());

    self.windowed = Some(WindowedRender::new(event_loop, &self.entry, &self.instance));
  }

  pub fn draw(&mut self) {}
}

fn create_window(event_loop: &EventLoop<()>) -> Window {
  WindowBuilder::new()
    .with_title(WINDOW_TITLE)
    .with_inner_size(LogicalSize::new(
      INITIAL_WINDOW_WIDTH,
      INITIAL_WINDOW_HEIGHT,
    ))
    .build(event_loop)
    .expect("Failed to create window.")
}

struct WindowedRender {
  window: Window,
  surface_loader: ash::extensions::khr::Surface,
  surface: vk::SurfaceKHR,
  renderer: SyncRenderer,
}

impl WindowedRender {
  pub fn new(event_loop: &EventLoop<()>, entry: &ash::Entry, instance: &ash::Instance) -> Self {
    let window = create_window(event_loop);

    let surface_loader = ash::extensions::khr::Surface::new(&entry, &instance);
    let surface = unsafe {
      ash_window::create_surface(
        entry,
        instance,
        event_loop.raw_display_handle(),
        window.raw_window_handle(),
        None,
      )
      .expect("Failed to create window surface")
    };

    let renderer = SyncRenderer::new(event_loop, instance);

    Self {
      window,
      surface_loader,
      surface,
      renderer,
    }
  }
}
