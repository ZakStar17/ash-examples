use raw_window_handle::HasRawDisplayHandle;
use winit::event_loop::EventLoop;

use super::renderer::Renderer;

const FRAMES_IN_FLIGHT: usize = 2;

pub struct SyncRenderer {
  renderer: Renderer,

  recreate_swapchain_next_frame: bool,
}

impl SyncRenderer {
  pub fn new(event_loop: &EventLoop<()>, instance: &ash::Instance) -> Self {
    let renderer = Renderer::new(event_loop, instance);

    Self {
      renderer,

      recreate_swapchain_next_frame: false,
    }
  }

  pub fn handle_window_resize(&mut self) {
    self.recreate_swapchain_next_frame = true;
  }

  pub fn render_next_frame(&mut self) {}
}

impl Drop for SyncRenderer {
  fn drop(&mut self) {
    unsafe {
      // for frame in self.frames.iter_mut() {
      //   frame.destroy(&self.renderer.device);
      // }
    }
  }
}
