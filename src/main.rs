mod app;
mod render;
mod utility;

use std::{
  ffi::CStr,
  time::{Duration, Instant},
};

use app::App;
use ash::vk;
use utility::cstr;
use winit::{
  event::{Event, MouseScrollDelta, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
};

pub const APPLICATION_NAME: &'static CStr = cstr!("Bouncy Ferris");
pub const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

pub const WINDOW_TITLE: &'static str = "Bouncy Ferris";
pub const INITIAL_WINDOW_WIDTH: u32 = 800;
pub const INITIAL_WINDOW_HEIGHT: u32 = 600;

pub fn main_loop(event_loop: EventLoop<()>, mut app: App) {
  let mut last_frame_instant = Instant::now();

  event_loop
    .run(move |event, target| match event {
      Event::Suspended => {
        // should completely pause the application
        log::debug!("Application suspended");
      }
      Event::Resumed => {
        if !app.started {
          log::debug!("Starting the application");
          app.start();
        } else {
          log::debug!("Application resumed");
        }
        // get window and request redraw
      }
      Event::LoopExiting => {
        log::info!("Application exiting");
      }
      Event::WindowEvent { event, .. } => match event {
        WindowEvent::CloseRequested => {
          target.exit();
        }
        WindowEvent::Occluded(occluded) => {
          log::debug!("Application occluded: {}", occluded);

          // there is no point of rendering if the user doesn't see it
          // however, the application can still do other things
        }
        WindowEvent::RedrawRequested => {
          log::info!("Rendering");
        }
        WindowEvent::Resized(dimensions) => {
          log::info!("Window resized");
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

  let app = App::new(&event_loop);
  main_loop(event_loop, app);
}
