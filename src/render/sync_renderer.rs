use std::{ops::BitOr, ptr};

use ash::vk;
use winit::dpi::PhysicalSize;

use crate::utility::populate_array_with_expression;

use super::{
  common_object_creations::create_timeline_semaphore, frame::Frame, initialization::Surface,
  push_constants::SpritePushConstants, renderer::Renderer, ENABLE_FRAME_DEBUGGING,
  FRAMES_IN_FLIGHT,
};

pub struct SyncRenderer {
  pub renderer: Renderer,
  frames: [Frame; FRAMES_IN_FLIGHT],
  last_frame_i: usize,
  timelines: [u64; 2],

  // last frame swapchain was recreated and so current frame resources are marked as old
  // having more than two frames in flight could require having more than one old set of resources
  last_frame_recreated_swapchain: bool,
  // will have the new window size
  recreate_swapchain_next_frame: bool,

  first_frame: bool,

  test: [vk::Semaphore; 2],
}

impl SyncRenderer {
  pub fn new(renderer: Renderer) -> Self {
    let frames = populate_array_with_expression!(Frame::new(&renderer.device), FRAMES_IN_FLIGHT);
    let test = [
      create_timeline_semaphore(&renderer.device, 0),
      create_timeline_semaphore(&renderer.device, 2),
    ];

    Self {
      renderer,
      frames,
      last_frame_i: 1,
      timelines: [0, 2],

      last_frame_recreated_swapchain: false,
      recreate_swapchain_next_frame: false,

      first_frame: true,

      test,
    }
  }

  pub fn render_next_frame(
    &mut self,
    surface: &Surface,
    window_size: PhysicalSize<u32>,
    extent_changed: bool,
    delta_time: f32,
    player: &SpritePushConstants,
  ) -> Result<(), ()> {
    if extent_changed {
      self.recreate_swapchain_next_frame = true;
    }

    let last_frame_i = self.last_frame_i;
    let cur_frame_i = (self.last_frame_i + 1) % FRAMES_IN_FLIGHT;
    let cur_frame: &Frame = &self.frames[cur_frame_i];
    let cur_test = self.test[cur_frame_i];
    let other_test = self.test[self.last_frame_i];
    self.last_frame_i = cur_frame_i;

    if ENABLE_FRAME_DEBUGGING {
      println!("timeline: {:?}, frame i: {}", self.timelines, cur_frame_i);
      let status = |val, is_cur| {
        if val % 2 == 0 {
          let time_val = if is_cur {
            self.timelines[cur_frame_i]
          } else {
            self.timelines[last_frame_i]
          };
          if val == time_val {
            "ALL"
          } else {
            "NOTHING"
          }
        } else {
          "COMPUTE"
        }
      };

      unsafe {
        let old = self
          .renderer
          .device
          .get_semaphore_counter_value(other_test)
          .unwrap();
        let cur = self
          .renderer
          .device
          .get_semaphore_counter_value(cur_test)
          .unwrap();
        println!(
          "BEFORE FENCE: Current: {} ({}), last: {} ({})",
          cur,
          status(cur, true),
          old,
          status(old, false)
        );
      }

      cur_frame.wait_graphics(&self.renderer.device);

      unsafe {
        let old = self
          .renderer
          .device
          .get_semaphore_counter_value(other_test)
          .unwrap();
        let cur = self
          .renderer
          .device
          .get_semaphore_counter_value(cur_test)
          .unwrap();
        println!(
          "AFTER  FENCE: Current: {} ({}), last: {} ({})",
          cur,
          status(cur, true),
          old,
          status(old, false)
        );
      }
    } else {
      cur_frame.wait_graphics(&self.renderer.device);
    }

    let (compute_record_data, bullet_instance_count) = self.renderer.compute_data.update(
      cur_frame_i,
      !self.first_frame,
      delta_time,
      player.position,
    );
    unsafe {
      self.renderer.compute_pools[cur_frame_i].reset(&self.renderer.device);
      self
        .renderer
        .record_compute(cur_frame_i, compute_record_data);
    }

    let wait_semaphores = [vk::SemaphoreSubmitInfo {
      s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
      p_next: ptr::null(),
      semaphore: other_test,
      value: self.timelines[last_frame_i] - 1,
      // wait any compute / copy that may be writing to the compute instance buffers
      // (in order for data to be synchronized without data races, only one of can execute at a time)
      stage_mask: vk::PipelineStageFlags2::COPY.bitor(vk::PipelineStageFlags2::COMPUTE_SHADER),
      device_index: 0,
    }];

    self.timelines[cur_frame_i] += 1;
    let signal_semaphores = [vk::SemaphoreSubmitInfo {
      s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
      p_next: ptr::null(),
      semaphore: cur_test,
      value: self.timelines[cur_frame_i],
      stage_mask: vk::PipelineStageFlags2::COPY.bitor(vk::PipelineStageFlags2::COMPUTE_SHADER),
      device_index: 0,
    }];

    let command_buffer_infos = [vk::CommandBufferSubmitInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_SUBMIT_INFO,
      p_next: ptr::null(),
      command_buffer: self.renderer.compute_pools[cur_frame_i].buffer,
      device_mask: 0,
    }];
    let submit_info = vk::SubmitInfo2 {
      s_type: vk::StructureType::SUBMIT_INFO_2,
      p_next: ptr::null(),
      flags: vk::SubmitFlags::empty(),
      wait_semaphore_info_count: wait_semaphores.len() as u32,
      p_wait_semaphore_infos: wait_semaphores.as_ptr(),
      signal_semaphore_info_count: signal_semaphores.len() as u32,
      p_signal_semaphore_infos: signal_semaphores.as_ptr(),
      command_buffer_info_count: command_buffer_infos.len() as u32,
      p_command_buffer_infos: command_buffer_infos.as_ptr(),
    };
    unsafe {
      self
        .renderer
        .device
        .queue_submit2(
          self.renderer.queues.compute,
          &[submit_info],
          vk::Fence::null(),
        )
        .unwrap();
    }

    self.first_frame = false;

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
      self.renderer.record_graphics(
        cur_frame_i,
        image_index as usize,
        bullet_instance_count as u32,
        player,
      );
    }

    let wait_semaphores = [
      vk::SemaphoreSubmitInfo {
        s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
        p_next: ptr::null(),
        semaphore: cur_test,
        value: self.timelines[cur_frame_i],
        stage_mask: vk::PipelineStageFlags2::COPY,
        device_index: 0,
      },
      vk::SemaphoreSubmitInfo {
        s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
        p_next: ptr::null(),
        semaphore: cur_frame.image_available,
        value: 0,
        stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        device_index: 0,
      },
    ];

    self.timelines[cur_frame_i] += 1;
    let signal_semaphores = [
      vk::SemaphoreSubmitInfo {
        s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
        p_next: ptr::null(),
        semaphore: cur_test,
        value: self.timelines[cur_frame_i],
        stage_mask: vk::PipelineStageFlags2::TRANSFER,
        device_index: 0,
      },
      vk::SemaphoreSubmitInfo {
        s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
        p_next: ptr::null(),
        semaphore: cur_frame.presentable,
        value: 0,
        stage_mask: vk::PipelineStageFlags2::BLIT,
        device_index: 0,
      },
    ];

    let command_buffer_infos = [vk::CommandBufferSubmitInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_SUBMIT_INFO,
      p_next: ptr::null(),
      command_buffer: self.renderer.graphics_pools[cur_frame_i].triangle,
      device_mask: 0,
    }];
    let submit_info = vk::SubmitInfo2 {
      s_type: vk::StructureType::SUBMIT_INFO_2,
      p_next: ptr::null(),
      flags: vk::SubmitFlags::empty(),
      wait_semaphore_info_count: wait_semaphores.len() as u32,
      p_wait_semaphore_infos: wait_semaphores.as_ptr(),
      signal_semaphore_info_count: signal_semaphores.len() as u32,
      p_signal_semaphore_infos: signal_semaphores.as_ptr(),
      command_buffer_info_count: command_buffer_infos.len() as u32,
      p_command_buffer_infos: command_buffer_infos.as_ptr(),
    };
    unsafe {
      self
        .renderer
        .device
        .queue_submit2(
          self.renderer.queues.graphics,
          &[submit_info],
          cur_frame.graphics_finished,
        )
        .unwrap();
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

    self.renderer.device.destroy_semaphore(self.test[0], None);
    self.renderer.device.destroy_semaphore(self.test[1], None);

    self.renderer.destroy_self();
  }
}
