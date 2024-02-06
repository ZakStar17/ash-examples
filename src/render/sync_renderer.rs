use ash::vk;

use super::renderer::Renderer;

const FRAMES_IN_FLIGHT: usize = 2;

pub struct SyncRenderer {
  renderer: Renderer,

  recreate_swapchain_next_frame: bool,
}

impl SyncRenderer {
  pub fn new(
    instance: &ash::Instance,
    surface_loader: &ash::extensions::khr::Surface,
    surface: vk::SurfaceKHR,
  ) -> Self {
    let renderer = Renderer::new(instance, surface_loader, surface);

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
