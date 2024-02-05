use winit::event_loop::EventLoop;

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

  pub fn resume(&mut self) {
    self.render.resume()
  }

  pub fn get_window(&self) {}
}
