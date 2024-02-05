use ash::vk;
use winit::window::Window;

pub struct WindowedRenderer {
  pub window: Window,
  surface: vk::SurfaceKHR,
}

// impl WindowedRenderer {
//     pub fn new() {
//         let
//     }
// }
