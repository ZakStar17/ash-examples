mod app;
mod render;
mod utility;

use std::ffi::CStr;

use app::App;
use ash::vk;
use render::RenderEngine;
use utility::cstr;
use winit::{
  event::{Event, StartCause, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
};

pub const APPLICATION_NAME: &'static CStr = cstr!("Bouncy Ferris");
pub const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

pub const WINDOW_TITLE: &'static str = "Bouncy Ferris";
pub const INITIAL_WINDOW_WIDTH: u32 = 800;
pub const INITIAL_WINDOW_HEIGHT: u32 = 800;

pub const USE_VSYNC: bool = true;

pub fn main_loop(event_loop: EventLoop<()>, mut engine: RenderEngine) {
  let mut started = false;

  event_loop
    .run(move |event, target| match event {
      Event::NewEvents(cause) => match cause {
        StartCause::Poll => engine.request_window_redraw(),
        _ => {}
      },
      Event::Suspended => {
        // should completely pause the application
        log::debug!("Application suspended");
      }
      Event::Resumed => {
        if !started {
          log::debug!("Starting the application");
          engine.start(target);
          started = true;
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
        }
        WindowEvent::RedrawRequested => {
          engine.render_frame();
        }
        WindowEvent::Resized(new_size) => {
          log::info!("Window resized to {:?}", new_size);
          engine.window_resized(new_size);
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
