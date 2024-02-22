mod player;
mod render;
mod utility;

use std::{
  ffi::CStr,
  time::{Duration, Instant},
};

use ash::vk;
use player::Player;
use render::{RenderEngine, ENABLE_FRAME_DEBUGGING};
use winit::{
  dpi::PhysicalSize,
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  keyboard::{KeyCode, PhysicalKey},
};

pub const APPLICATION_NAME: &'static CStr = cstr!("Bouncy Ferris");
pub const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

pub const WINDOW_TITLE: &'static str = "Bouncy Ferris";
pub const INITIAL_WINDOW_WIDTH: u32 = 800;
pub const INITIAL_WINDOW_HEIGHT: u32 = 800;

pub const RESOLUTION: [u32; 2] = [800, 800];

// see https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPresentModeKHR.html
// FIFO_KHR is required to be supported and corresponds as to enabling VSync in games
// IMMEDIATE will be chosen over RELAXED_KHR if the latter is not supported
// otherwise, presentation mode will fallback to FIFO_KHR
pub const PREFERRED_PRESENTATION_METHOD: vk::PresentModeKHR = vk::PresentModeKHR::IMMEDIATE;

// This application doesn't use dynamic pipeline size, so resizing is expensive
// If a small resize happens (for example while resizing with the mouse) this usually means that
// more are to come, and recreating objects each frame can make the application unresponsive
// If enabled, the render function will sleep for some time and wait for more window resize events
// to be acknowledged
const WAIT_AFTER_WINDOW_RESIZE_ENABLED: bool = true;
const WAIT_AFTER_WINDOW_RESIZE_THRESHOLD: u32 = 20;
const WAIT_AFTER_WINDOW_RESIZE_DURATION: Duration = Duration::from_millis(60);

// prints current frame 1 / <time since last frame> every x time
const PRINT_FPS_EVERY: Duration = Duration::from_millis(1000);

pub fn main_loop(event_loop: EventLoop<()>, mut engine: RenderEngine) {
  let mut started = false;
  let mut engine_running = false;

  let mut wait_for_more_window_resizes = false;
  let mut cur_window_size = PhysicalSize {
    width: u32::MAX,
    height: u32::MAX,
  };

  let mut player = Player::new([0.0, 0.0]);

  let mut last_update_instant = Instant::now();
  let mut time_since_last_fps_print = Duration::ZERO;

  let mut frame_i: usize = 0;
  event_loop
    .run(move |event, target| match event {
      Event::Suspended => {
        // should completely pause the application
        log::debug!("Application suspended");
        engine_running = false;
      }
      Event::Resumed => {
        if !started {
          log::debug!("Starting application");
          cur_window_size = engine.start(target);
          started = true;
        } else {
          log::debug!("Application resumed");
        }
        engine_running = true;
      }
      Event::AboutToWait => {
        // As of time of writing the Winit docs contradict themselves about when and how to use
        // Event::AboutToWait and WindowEvent::RedrawRequested. Based on my tests RedrawRequested
        // works well if the application doesn't draw regularly but makes the application lag and
        // be generally unresponsive if it is called multiple times per frame (for example by using
        // window.request_redraw()), specially when PREFERRED_PRESENTATION_METHOD is IMMEDIATE.

        // Doing everything in Event::AboutToWait seems to work best as the application doesn't
        // have to wait for anything and can resume rendering when it wants. This works well
        // because the renderer is synchronized with the GPU anyway. This way
        // vk::PresentModeKHR::IMMEDIATE uses all resources available and works as intended.

        let now = Instant::now();
        let time_passed = now - last_update_instant;
        last_update_instant = now;

        time_since_last_fps_print += time_passed;
        if time_since_last_fps_print >= PRINT_FPS_EVERY {
          time_since_last_fps_print -= PRINT_FPS_EVERY;
          println!("FPS: {}", 1.0 / time_passed.as_secs_f32());
        }

        player.update(time_passed.as_secs_f32());

        if wait_for_more_window_resizes {
          wait_for_more_window_resizes = false;
          std::thread::sleep(WAIT_AFTER_WINDOW_RESIZE_DURATION);
          // return so the loop can register new events
          return;
        }

        if ENABLE_FRAME_DEBUGGING {
          log::debug!("\n---------------\nFRAME: {}\n---------------", frame_i);
        }
        if engine_running {
          if engine
            .render_frame(time_passed.as_secs_f32(), &player.sprite_data())
            .is_err()
          {
            log::warn!("Frame failed to render");
          }
        }

        // if frame_i == 8 {
        //   target.exit();
        // }
        frame_i = frame_i.wrapping_add_signed(1);
      }
      Event::WindowEvent { event, .. } => match event {
        WindowEvent::CloseRequested => {
          target.exit();
        }
        WindowEvent::Occluded(occluded) => {
          engine_running = !occluded;
        }
        WindowEvent::Resized(new_size) => {
          engine.window_resized(new_size);

          if WAIT_AFTER_WINDOW_RESIZE_ENABLED
            && (cur_window_size.width.abs_diff(new_size.width)
              <= WAIT_AFTER_WINDOW_RESIZE_THRESHOLD
              || cur_window_size.height.abs_diff(new_size.height)
                <= WAIT_AFTER_WINDOW_RESIZE_THRESHOLD)
          {
            wait_for_more_window_resizes = true;
          }

          cur_window_size = new_size;
        }
        WindowEvent::KeyboardInput { event, .. } => {
          let pressed = event.state.is_pressed();
          match event.physical_key {
            // close on escape
            PhysicalKey::Code(code) => match code {
              KeyCode::Escape => target.exit(),
              KeyCode::Pause => {
                todo!();
              }
              KeyCode::ArrowUp | KeyCode::KeyW | KeyCode::KeyK => {
                if pressed {
                  player.up_press();
                } else {
                  player.up_release();
                }
              }
              KeyCode::ArrowDown | KeyCode::KeyS | KeyCode::KeyJ => {
                if pressed {
                  player.down_press();
                } else {
                  player.down_release();
                }
              }
              KeyCode::ArrowLeft | KeyCode::KeyA | KeyCode::KeyH => {
                if pressed {
                  player.left_press();
                } else {
                  player.left_release();
                }
              }
              KeyCode::ArrowRight | KeyCode::KeyD | KeyCode::KeyL => {
                if pressed {
                  player.right_press();
                } else {
                  player.right_release();
                }
              }
              _ => {}
            },
            _ => {}
          }
        }
        _ => {}
      },
      _ => (),
    })
    .expect("Failed to run event loop")
}

fn main() {
  env_logger::init();
  let event_loop = EventLoop::new().expect("Failed to initialize event loop");

  // make the event loop run continuously even if there is no new user input
  event_loop.set_control_flow(ControlFlow::Poll);

  let render = RenderEngine::init(&event_loop);
  main_loop(event_loop, render);
}
