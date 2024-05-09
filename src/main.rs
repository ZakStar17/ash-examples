#![feature(vec_into_raw_parts)]

mod render;
mod utility;

use ash::vk;
use render::{RenderInit, SyncRenderer};
use std::{
  ffi::CStr,
  mem::{self, MaybeUninit},
  time::{Duration, Instant},
};
use winit::{
  dpi::PhysicalSize,
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
  keyboard::{KeyCode, PhysicalKey},
};

const APPLICATION_NAME: &'static CStr = cstr!("Bouncy Ferris");
const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

const WINDOW_TITLE: &'static str = "Bouncy Ferris";
const INITIAL_WINDOW_WIDTH: u32 = 800;
const INITIAL_WINDOW_HEIGHT: u32 = 800;

const BACKGROUND_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [0.01, 0.01, 0.01, 1.0],
};

const TEXTURE_PATH: &'static str = "./ferris.png";

// see https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPresentModeKHR.html
// FIFO_KHR is required to be supported and functions as vsync
// IMMEDIATE will be chosen over RELAXED_KHR if the latter is not supported
// otherwise, presentation mode will fallback to FIFO_KHR
const PREFERRED_PRESENTATION_METHOD: vk::PresentModeKHR = vk::PresentModeKHR::IMMEDIATE;

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

enum RenderStatus {
  Initialized(RenderInit),
  Started(SyncRenderer),
}
enum Status {
  NotStarted(RenderInit),
  Running(SyncRenderer),
  Paused(SyncRenderer),
}

impl Status {
  pub fn running(&self) -> bool {
    if let Self::Running(_) = self {
      true
    } else {
      false
    }
  }

  pub fn start(&mut self, event_loop: &EventLoopWindowTarget<()>) {
    take_mut::take(self, |old| match old {
      Self::NotStarted(init) => {
        let renderer = init.start(event_loop);
        Status::Running(renderer)
      }
      _ => panic!(),
    });
  }

  pub fn set_to_running(&mut self) {
    take_mut::take(self, |old| match old {
      Self::Paused(renderer) => Self::Running(renderer),
      _ => panic!(),
    });
  }

  pub fn set_to_paused(&mut self) {
    take_mut::take(self, |old| match old {
      Self::Running(renderer) => Self::Paused(renderer),
      _ => panic!(),
    });
  }
}

impl PartialEq for Status {
  fn eq(&self, other: &Self) -> bool {
    match self {
      Status::NotStarted(_) => {
        if let Status::NotStarted(_) = other {
          return true;
        }
      }
      Status::Running(_) => {
        if let Status::Running(_) = other {
          return true;
        }
      }
      Status::Paused(_) => {
        if let Status::Paused(_) = other {
          return true;
        }
      }
    }
    false
  }
}

pub fn main_loop(event_loop: EventLoop<()>, init: RenderInit) {
  let mut status = Status::NotStarted(init);

  let mut wait_for_more_window_resizes = false;
  let mut cur_window_size = PhysicalSize {
    width: u32::MAX,
    height: u32::MAX,
  };

  let mut last_update_instant = Instant::now();
  let mut time_since_last_fps_print = Duration::ZERO;

  let mut frame_i: usize = 0;
  event_loop
    .run(move |event, target| match event {
      Event::Suspended => {
        // should completely pause the application
        log::debug!("Application suspended");
        if status.running() {
          status.set_to_paused();
        }
      }
      Event::Resumed => match &mut status {
        Status::NotStarted(_) => {
          log::debug!("Starting application");

          status.start(target);

          target.exit() // todo: debugging
        }
        Status::Paused(_) => status.set_to_running(),
        Status::Running(_) => {}
      },
      Event::AboutToWait => {
        // winit has two events that notify when a frame needs to be rendered:
        // WindowEvent::RedrawRequested => Useful for applications that don't render often,
        //  triggers only if the system requests for a rerender (for example during window resize)
        //  or if a redraw is requested explicitly (using window.request_redraw()).
        // Event::AboutToWait => Triggered instantly after the previous event once new inputs have
        // been processed. Useful for applications that draw continuously.

        let now = Instant::now();
        let time_passed = now - last_update_instant;
        last_update_instant = now;

        time_since_last_fps_print += time_passed;
        if time_since_last_fps_print >= PRINT_FPS_EVERY {
          time_since_last_fps_print -= PRINT_FPS_EVERY;
          println!("FPS: {}", 1.0 / time_passed.as_secs_f32());
        }

        if wait_for_more_window_resizes {
          wait_for_more_window_resizes = false;
          std::thread::sleep(WAIT_AFTER_WINDOW_RESIZE_DURATION);
          // return so the loop can register new events
          return;
        }

        if let Status::Running(renderer) = &mut status {
          renderer.render_next_frame();
          // if engine
          //   .render_frame(time_passed.as_secs_f32(), &player.sprite_data())
          //   .is_err()
          // {
          //   log::warn!("Frame failed to render");
          // }
        }
      }
      Event::WindowEvent { event, .. } => match event {
        WindowEvent::CloseRequested => {
          target.exit();
        }
        // WindowEvent::Occluded(occluded) => match status {
        //   Status::NotStarted(init) => {
        //     log::debug!("Starting application");
        //     status = Status::Running;
        //   }
        //   Status::Paused => {
        //     if !occluded {
        //       status = Status::Running;
        //     }
        //   }
        //   Status::Running => {
        //     if occluded {
        //       status = Status::Paused;
        //     }
        //   }
        // },
        WindowEvent::Resized(new_size) => {
          match &mut status {
            Status::Running(renderer) | Status::Paused(renderer) => renderer.window_resized(),
            _ => {}
          }

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

  let init = RenderInit::new(&event_loop).expect("Failed to initialize before window");
  main_loop(event_loop, init);
}
