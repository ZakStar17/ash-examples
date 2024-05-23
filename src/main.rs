#![feature(vec_into_raw_parts)]

mod render;
mod utility;

use ash::vk;
use render::{RenderInit, RenderInitError, SyncRenderer};
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

const APPLICATION_NAME: &CStr = c"Bouncy Ferris";
const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

const WINDOW_TITLE: &str = "Bouncy Ferris";
const INITIAL_WINDOW_WIDTH: u32 = 800;
const INITIAL_WINDOW_HEIGHT: u32 = 800;

const BACKGROUND_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [0.01, 0.01, 0.01, 1.0],
};

const TEXTURE_PATH: &str = "./ferris.png";

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
enum RunningStatus {
  NotStarted,
  Running,
  Paused,
}

struct ProgramStatus {
  render: RenderStatus,
  running: RunningStatus,
}

impl ProgramStatus {
  pub fn new(event_loop: &EventLoop<()>) -> Result<Self, RenderInitError> {
    let render = RenderInit::new(event_loop)?;
    Ok(Self {
      render: RenderStatus::Initialized(render),
      running: RunningStatus::NotStarted,
    })
  }

  pub fn start(&mut self, event_loop: &EventLoopWindowTarget<()>) {
    take_mut::take(&mut self.render, |old| match old {
      RenderStatus::Initialized(init) => {
        let renderer = init.start(event_loop);
        RenderStatus::Started(renderer)
      }
      _ => panic!("Render started multiple times"),
    });
    self.running = RunningStatus::Running;
  }

  pub fn pause(&mut self) {
    if let RunningStatus::Running = self.running {
      self.running = RunningStatus::Paused;
    }
  }

  pub fn unpause(&mut self) {
    if let RunningStatus::Paused = self.running {
      self.running = RunningStatus::Running;
    }
  }

  pub fn is_running(&self) -> bool {
    if let RunningStatus::Running = self.running {
      true
    } else {
      false
    }
  }

  pub fn has_started(&self) -> bool {
    if let RunningStatus::NotStarted = self.running {
      false
    } else {
      true
    }
  }

  pub fn get_renderer(&mut self) -> &mut SyncRenderer {
    if let RenderStatus::Started(render) = &mut self.render {
      render
    } else {
      panic!();
    }
  }
}

fn main_loop(event_loop: EventLoop<()>, mut status: ProgramStatus) {
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
        status.pause();
      }
      Event::Resumed => {
        if status.has_started() {
          status.unpause();
        } else {
          log::debug!("Starting application");

          status.start(target);

          //target.exit() // todo: debugging
        }
      }
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

        if status.is_running() {
          if frame_i < 300 {
            println!("\n\nRENDERING FRAME {}\n", frame_i);
            status.get_renderer().render_next_frame();
          }
          frame_i += 1;
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
          if status.has_started() {
            status.get_renderer().window_resized();
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

  let status = ProgramStatus::new(&event_loop).unwrap(); // todo
  main_loop(event_loop, status);
}
