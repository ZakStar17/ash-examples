use winit::dpi::PhysicalSize;

use crate::render::RenderPosition;

pub struct Ferris {
  pub position: [u32; 2]
}

impl Ferris {
  // width and height in pixels
  pub const SIZE: u32 = 80;

  pub fn new(position: [u32; 2]) -> Self {
    Self {
      position
    }
  }

  pub fn get_render_position(&self, window_size: PhysicalSize<u32>) -> RenderPosition {
    let window_size_float = PhysicalSize {
      width: window_size.width as f32,
      height: window_size.height as f32
    };

    let normal_pos = [self.position[0] as f32 / window_size_float.width, self.position[1] as f32 / window_size_float.height];

    let zoom = Ferris::SIZE as f32 / window_size_float.width;  // todo

    RenderPosition::new(normal_pos, zoom)
  }
}
