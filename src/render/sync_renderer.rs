use std::ptr;

use ash::vk;
use winit::dpi::PhysicalSize;

use crate::utility::populate_array_with_expression;

use super::{
  frame::Frame, objects::Surface, push_constants::SpritePushConstants, renderer::Renderer,
  ComputeOutput, FRAMES_IN_FLIGHT,
};

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

  pub fn render_next_frame(
    &mut self,
    surface: &Surface,
    window_size: PhysicalSize<u32>,
    extent_changed: bool,
    player: &SpritePushConstants,
  ) -> Result<(), ()> {
    if extent_changed {
      self.recreate_swapchain_next_frame = true;
    }

    let cur_frame_i = (self.last_frame_i + 1) % FRAMES_IN_FLIGHT;
    let cur_frame: &Frame = &self.frames[cur_frame_i];
    self.last_frame_i = cur_frame_i;

    cur_frame.wait_finished(&self.renderer.device);

    // current frame resources are now safe to use as they are not being used by the GPU

    // compute

    let data = ComputeOutput { collision: 0 };

    unsafe {
      let mut mem_ptr = self
        .renderer
        .device
        .map_memory(
          self.renderer.compute_output_memory,
          0,
          vk::WHOLE_SIZE,
          vk::MemoryMapFlags::empty(),
        )
        .expect("...") as *mut u8;

      if cur_frame_i == 1 {
        mem_ptr = mem_ptr.byte_add(self.renderer.compute_offset);
      }

      let old = (mem_ptr as *const ComputeOutput).as_ref().unwrap();
      if old.collision > 0 {
        println!("colliding");
      }

      std::ptr::copy_nonoverlapping(
        ptr::addr_of!(data) as *mut u8,
        mem_ptr,
        std::mem::size_of::<ComputeOutput>(),
      );

      self
        .renderer
        .device
        .unmap_memory(self.renderer.compute_output_memory);
    }

    unsafe {
      self.renderer.compute_pools[cur_frame_i].reset(&self.renderer.device);
      self.renderer.record_compute(cur_frame_i, player);
    }

    let submit_info = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 0,
      p_wait_semaphores: ptr::null(),
      p_wait_dst_stage_mask: ptr::null(),
      command_buffer_count: 1,
      p_command_buffers: &self.renderer.compute_pools[cur_frame_i].buffer,
      signal_semaphore_count: 1,
      p_signal_semaphores: &cur_frame.compute_finished,
    };
    unsafe {
      self
        .renderer
        .device
        .queue_submit(
          self.renderer.queues.compute,
          &[submit_info],
          vk::Fence::null(),
        )
        .expect("Failed to submit to queue");
    }

    // graphics

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
        .record_graphics(cur_frame_i, image_index as usize, player);
    }

    let wait_semaphores = [cur_frame.compute_finished, cur_frame.image_available];
    let wait_stages = [
      vk::PipelineStageFlags::COMPUTE_SHADER,
      vk::PipelineStageFlags::TRANSFER,
    ];
    let submit_info = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: wait_semaphores.len() as u32,
      p_wait_semaphores: wait_semaphores.as_ptr(),
      p_wait_dst_stage_mask: wait_stages.as_ptr(),
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
      if let Err(vk_result) = self.renderer.swapchains.queue_present(
        image_index,
        self.renderer.queues.presentation,
        &[cur_frame.presentable],
      ) {
        match vk_result {
          vk::Result::ERROR_OUT_OF_DATE_KHR => {
            // window resizes can happen while this function is running and be not detected in time
            // other reasons may include format changes

            log::warn!("Failed to present to swapchain: Swapchain is out of date");
            self.recreate_swapchain_next_frame = true;

            // errors of this type still signal sync objects accordingly
            return Err(());
          }
          other => panic!("Failed to present to swapchain: {:?}", other),
        }
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
