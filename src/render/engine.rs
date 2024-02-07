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
  RenderPosition,
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

  pub fn start(&mut self, target: &EventLoopWindowTarget<()>) -> PhysicalSize<u32> {
    assert!(self.windowed.is_none());

    let (windowed, initial_window_size) = WindowedRender::new(target, &self.entry, &self.instance);
    self.windowed = Some(windowed);

    initial_window_size
  }

  pub fn render_frame(&mut self, position: &RenderPosition) -> Result<(), ()> {
    self.windowed.as_mut().unwrap().render_next_frame(position)
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
  _window: Window,
  window_size: PhysicalSize<u32>,
  surface: Surface,
  pub sync: SyncRenderer,

  extent_may_have_changed: bool,
}

impl WindowedRender {
  pub fn new(
    target: &EventLoopWindowTarget<()>,
    entry: &ash::Entry,
    instance: &ash::Instance,
  ) -> (Self, PhysicalSize<u32>) {
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

    (
      Self {
        _window: window,
        window_size: initial_size,
        surface,
        sync: sync_renderer,

        extent_may_have_changed: false,
      },
      initial_size,
    )
  }

  pub fn render_next_frame(&mut self, position: &RenderPosition) -> Result<(), ()> {
    let mut extent_changed = false;

    if self.extent_may_have_changed {
      self.extent_may_have_changed = false;

      let capabilities = unsafe {
        self
          .surface
          .get_capabilities(*self.sync.renderer.physical_device)
      };
      let new_extent = Surface::get_extent_from_capabilities(&capabilities);
      if new_extent.is_some_and(|extent| self.sync.renderer.swapchains.get_extent() != extent) {
        extent_changed = true
      }
    }

    self
      .sync
      .render_next_frame(&self.surface, self.window_size, extent_changed, position)
  }

  pub fn window_resized(&mut self, new_size: PhysicalSize<u32>) {
    if new_size != self.window_size {
      self.window_size = new_size;
      self.extent_may_have_changed = true;
    }
  }

  pub unsafe fn destroy_self(&mut self) {
    self.sync.destroy_self();
    self.surface.destroy_self();
  }
}
