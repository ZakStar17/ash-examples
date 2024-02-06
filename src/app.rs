use winit::{
  dpi::PhysicalSize,
  event_loop::{EventLoop, EventLoopWindowTarget},
};

use crate::render::RenderEngine;

pub struct App {
  pub render: RenderEngine,
}

impl App {
  pub fn new(event_loop: &EventLoop<()>) -> Self {
    Self {
      render: RenderEngine::init(event_loop),
    }
  }

  pub fn request_redraw(&mut self) {}

  pub fn window_resized(&mut self, new_size: PhysicalSize<u32>) {
    self.render.window_resized(new_size);
  }
}
