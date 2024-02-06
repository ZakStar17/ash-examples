use winit::{
  dpi::PhysicalSize,
  event_loop::{EventLoop, EventLoopWindowTarget},
};

use crate::render::Render;

pub struct App {
  pub render: Render,
}

impl App {
  pub fn new(event_loop: &EventLoop<()>) -> Self {
    Self {
      render: Render::init(event_loop),
    }
  }

  pub fn start(&mut self, target: &EventLoopWindowTarget<()>) {
    self.render.start(target)
  }

  pub fn resume(&mut self) {
    self.render.render_frame();
  }

  pub fn window_resized(&mut self, new_size: PhysicalSize<u32>) {
    self.render.window_resized(new_size);
  }
}
