use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::{
  dpi::PhysicalSize,
  event_loop::{EventLoop, EventLoopWindowTarget},
  window::{Window, WindowBuilder},
};

use crate::{INITIAL_WINDOW_HEIGHT, INITIAL_WINDOW_WIDTH, WINDOW_TITLE};

#[cfg(feature = "vl")]
use super::objects::DebugUtils;
use super::{
  objects::{create_instance, get_entry, Surface},
  renderer::Renderer,
  sync_renderer::SyncRenderer,
};

pub struct RenderEngine {
  entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: DebugUtils,

  windowed: Option<WindowedRender>,
}

impl RenderEngine {
  pub fn init(event_loop: &EventLoop<()>) -> Self {
    let entry: ash::Entry = unsafe { get_entry() };

    #[cfg(feature = "vl")]
    let (instance, debug_utils) = create_instance(&entry, event_loop.raw_display_handle());
    #[cfg(not(feature = "vl"))]
    let instance = create_instance(&entry, event_loop.raw_display_handle());

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

    self.windowed = Some(WindowedRender::new(target, &self.entry, &self.instance));
  }

  pub fn render_frame(&mut self) {
    let retries = 7;
    let windowed = self.windowed.as_mut().unwrap();
    for _ in 0..retries {
      if windowed.render_next_frame().is_ok() {
        return;
      }
    }

    panic!("Failed to render frame multiple consecutive times");
  }

  pub fn request_window_redraw(&self) {
    self.windowed.as_ref().unwrap().window.request_redraw();
  }

  pub fn window_resized(&mut self, new_size: PhysicalSize<u32>) {
    self.windowed.as_mut().unwrap().window_resized(new_size);
  }
}

impl Drop for RenderEngine {
  fn drop(&mut self) {
    unsafe {
      if let Some(windowed) = self.windowed.as_mut() {
        windowed.destroy_self();
      }

      #[cfg(feature = "vl")]
      self.debug_utils.destroy_self();

      self.instance.destroy_instance(None);
    }
  }
}

fn create_window(target: &EventLoopWindowTarget<()>, initial_size: PhysicalSize<u32>) -> Window {
  WindowBuilder::new()
    .with_title(WINDOW_TITLE)
    .with_inner_size(initial_size)
    .build(target)
    .expect("Failed to create window.")
}

struct WindowedRender {
  pub window: Window,
  window_size: PhysicalSize<u32>,
  surface: Surface,
  pub sync: SyncRenderer,
}

impl WindowedRender {
  pub fn new(
    target: &EventLoopWindowTarget<()>,
    entry: &ash::Entry,
    instance: &ash::Instance,
  ) -> Self {
    let initial_size = PhysicalSize {
      width: INITIAL_WINDOW_WIDTH,
      height: INITIAL_WINDOW_HEIGHT,
    };

    let window = create_window(target, initial_size);

    let surface = Surface::new(
      entry,
      instance,
      target.raw_display_handle(),
      window.raw_window_handle(),
    );

    let renderer = Renderer::new(instance, &surface, initial_size);
    let sync_renderer = SyncRenderer::new(renderer);

    Self {
      window,
      window_size: initial_size,
      surface,
      sync: sync_renderer,
    }
  }

  pub fn render_next_frame(&mut self) -> Result<(), ()> {
    self.sync.render_next_frame(&self.surface, self.window_size)
  }

  pub fn window_resized(&mut self, new_size: PhysicalSize<u32>) {
    if new_size != self.window_size {
      self.window_size = new_size;

      let capabilities = unsafe {
        self
          .surface
          .get_capabilities(*self.sync.renderer.physical_device)
      };
      let new_extent = Surface::get_extent_from_capabilities(&capabilities);
      if new_extent.is_some_and(|extent| self.sync.renderer.swapchains.get_extent() != extent) {
        self.sync.extent_changed();
      }
    }
  }

  pub unsafe fn destroy_self(&mut self) {
    self.sync.destroy_self();
    self.surface.destroy_self();
  }
}
