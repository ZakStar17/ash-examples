use std::ptr;

use ash::vk;
use winit::dpi::PhysicalSize;

use crate::utility::populate_array_with_expression;

use super::{frame::Frame, objects::Surface, renderer::Renderer, FRAMES_IN_FLIGHT};

pub struct SyncRenderer {
  pub renderer: Renderer,
  frames: [Frame; FRAMES_IN_FLIGHT],
  last_frame_i: usize,

  // last frame swapchain was recreated and so current frame resources are marked as old
  // having more than two frames in flight could require having more than one old set of resources
  last_frame_recreated_swapchain: bool,
  // will have the new window size
  recreate_swapchain_next_frame: bool,
}

impl SyncRenderer {
  pub fn new(renderer: Renderer) -> Self {
    let frames = populate_array_with_expression!(Frame::new(&renderer.device), FRAMES_IN_FLIGHT);

    Self {
      renderer,
      frames,
      last_frame_i: 0,

      last_frame_recreated_swapchain: false,
      recreate_swapchain_next_frame: false,
    }
  }

  pub fn extent_changed(&mut self) {
    self.recreate_swapchain_next_frame = true;
  }

  pub fn render_next_frame(
    &mut self,
    surface: &Surface,
    window_size: PhysicalSize<u32>,
  ) -> Result<(), ()> {
    let cur_frame_i = (self.last_frame_i + 1) % FRAMES_IN_FLIGHT;
    let cur_frame = &self.frames[cur_frame_i];
    self.last_frame_i = cur_frame_i;

    cur_frame.wait_fence(&self.renderer.device);

    // current frame resources are now safe to use as they are not being used by the GPU

    if self.last_frame_recreated_swapchain {
      unsafe { self.renderer.destroy_old() }
      self.last_frame_recreated_swapchain = false;
    }

    if self.recreate_swapchain_next_frame {
      unsafe {
        self.renderer.recreate_swapchain(surface, window_size);
      }
      self.recreate_swapchain_next_frame = false;
      self.last_frame_recreated_swapchain = true;
    }

    let image_index = match unsafe {
      self
        .renderer
        .swapchains
        .acquire_next_image(cur_frame.image_available)
    } {
      Ok((image_index, suboptimal)) => {
        if suboptimal {
          self.recreate_swapchain_next_frame = true;
        }
        image_index
      }
      Err(_) => {
        log::warn!("Failed to acquire next swapchain image");
        self.recreate_swapchain_next_frame = true;

        return Err(());
      }
    };

    // actual rendering

    unsafe {
      self.renderer.graphics_pools[cur_frame_i].reset(&self.renderer.device);

      self
        .renderer
        .record_graphics(cur_frame_i, image_index as usize);
    }

    let wait_stage = vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
    let submit_info = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 1,
      p_wait_semaphores: &cur_frame.image_available,
      p_wait_dst_stage_mask: &wait_stage,
      command_buffer_count: 1,
      p_command_buffers: &self.renderer.graphics_pools[cur_frame_i].triangle,
      signal_semaphore_count: 1,
      p_signal_semaphores: &cur_frame.presentable,
    };
    unsafe {
      self
        .renderer
        .device
        .queue_submit(
          self.renderer.queues.graphics,
          &[submit_info],
          cur_frame.finished,
        )
        .expect("Failed to submit to queue");
    }

    unsafe {
      // the window may resize or the swapchain may become invalid while this function runs
      if let Err(_) = self.renderer.swapchains.queue_present(
        image_index,
        self.renderer.queues.presentation,
        &[cur_frame.presentable],
      ) {
        log::warn!("Failed to present to swapchain");
        self.recreate_swapchain_next_frame = true;

        return Err(());
      }
    }

    Ok(())
  }

  pub unsafe fn destroy_self(&mut self) {
    self
      .renderer
      .device
      .device_wait_idle()
      .expect("Failed to wait for device idleness while destroying resources");

    for frame in self.frames.iter_mut() {
      frame.destroy_self(&self.renderer.device);
    }

    self.renderer.destroy_self();
  }
}
