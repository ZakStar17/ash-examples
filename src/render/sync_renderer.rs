use ash::vk;

use super::{objects::Surface, renderer::Renderer};

const FRAMES_IN_FLIGHT: usize = 2;

pub struct SyncRenderer {
  renderer: Renderer,

  recreate_swapchain_next_frame: bool,
}

impl SyncRenderer {
  pub fn new(renderer: Renderer) -> Self {
    Self {
      renderer,

      recreate_swapchain_next_frame: false,
    }
  }

  pub fn handle_window_resize(&mut self) {
    self.recreate_swapchain_next_frame = true;
  }

  pub fn render_next_frame(&mut self) {}

  pub unsafe fn destroy_self(&mut self) {
    self.renderer.destroy_self();
  }
}
