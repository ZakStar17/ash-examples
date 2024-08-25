use std::{marker::PhantomData, ptr};

use ash::vk;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{ferris::Ferris, utility::OnErr, RESOLUTION};

use super::{
  create_objs::{create_semaphore, create_timeline_semaphore},
  device_destroyable::DeviceManuallyDestroyed,
  errors::InitializationError,
  renderer::Renderer,
  FrameRenderError, FRAMES_IN_FLIGHT,
};

pub struct SyncRenderer {
  pub renderer: Renderer,
  last_frame_i: usize,
  timeline: vk::Semaphore,
  timeline_index: u64,

  // swapchain semaphores
  image_available: [vk::Semaphore; FRAMES_IN_FLIGHT],
  presentable: [vk::Semaphore; FRAMES_IN_FLIGHT],

  // last frame swapchain was recreated and so current frame resources are marked as old
  // having more than two frames in flight could require having more than one old set of resources
  last_frame_recreated_swapchain: bool,
  // will have the new window size
  recreate_swapchain_next_frame: bool,
}

impl SyncRenderer {
  // by how much timeline_index is incremented each frame
  const PER_FRAME_TIMELINE_INCREMENT: u64 = 1;

  pub fn new(renderer: Renderer) -> Result<Self, InitializationError> {
    let initial_timeline_index = 0;
    let timeline = create_timeline_semaphore(&renderer.device, initial_timeline_index)?;

    let available_sem_1 = create_semaphore(&renderer.device)
      .on_err(|_| unsafe { timeline.destroy_self(&renderer.device) })?;
    let available_sem_2 = create_semaphore(&renderer.device).on_err(|_| unsafe {
      timeline.destroy_self(&renderer.device);
      available_sem_1.destroy_self(&renderer.device);
    })?;
    let image_available = [available_sem_1, available_sem_2];

    let presentable_sem_1 = create_semaphore(&renderer.device).on_err(|_| unsafe {
      timeline.destroy_self(&renderer.device);
      image_available.destroy_self(&renderer.device);
    })?;
    let presentable_sem_2 = create_semaphore(&renderer.device).on_err(|_| unsafe {
      timeline.destroy_self(&renderer.device);
      image_available.destroy_self(&renderer.device);
      presentable_sem_1.destroy_self(&renderer.device);
    })?;
    let presentable = [presentable_sem_1, presentable_sem_2];

    Ok(Self {
      renderer,
      last_frame_i: 1,
      timeline,
      timeline_index: initial_timeline_index,

      image_available,
      presentable,

      last_frame_recreated_swapchain: false,
      recreate_swapchain_next_frame: false,
    })
  }

  pub fn window_resized(&mut self) {
    self.recreate_swapchain_next_frame = true;
  }

  pub fn window(&self) -> &Window {
    &self.renderer.window
  }

  pub fn render_next_frame(&mut self, ferris: &Ferris) -> Result<(), FrameRenderError> {
    let cur_frame_i = (self.last_frame_i + 1) % FRAMES_IN_FLIGHT;
    self.last_frame_i = cur_frame_i;
    let next_timeline_value = self.timeline_index + Self::PER_FRAME_TIMELINE_INCREMENT;

    // wait for last frame to finish rendering
    unsafe {
      let wait_info = vk::SemaphoreWaitInfo {
        s_type: vk::StructureType::SEMAPHORE_WAIT_INFO,
        p_next: ptr::null(),
        flags: vk::SemaphoreWaitFlags::empty(),
        semaphore_count: 1,
        p_semaphores: &self.timeline,
        p_values: &self.timeline_index,
        _marker: PhantomData,
      };
      self.renderer.device.wait_semaphores(&wait_info, u64::MAX)?
    }

    // current frame resources are now safe to use as they are not being used by the GPU

    if self.last_frame_recreated_swapchain {
      unsafe { self.renderer.destroy_old() }
      self.last_frame_recreated_swapchain = false;
    }

    if self.recreate_swapchain_next_frame {
      unsafe {
        self.renderer.recreate_swapchain()?;
      }
      self.recreate_swapchain_next_frame = false;
      self.last_frame_recreated_swapchain = true;
    }

    let image_index = match unsafe {
      self
        .renderer
        .swapchains
        .acquire_next_image(self.image_available[cur_frame_i])
    } {
      Ok((image_index, suboptimal)) => {
        if suboptimal {
          self.recreate_swapchain_next_frame = true;
        }
        image_index
      }
      Err(err) => {
        log::warn!("Failed to acquire next swapchain image");
        self.recreate_swapchain_next_frame = true;

        return Err(err.into());
      }
    };

    // actual rendering

    unsafe {
      self.renderer.record_graphics(
        cur_frame_i,
        image_index as usize,
        &ferris.get_render_position(PhysicalSize {
          width: RESOLUTION[0],
          height: RESOLUTION[1],
        }),
      )?;
    }

    let command_buffers = [vk::CommandBufferSubmitInfo::default()
      .command_buffer(self.renderer.command_pools[cur_frame_i].main)];
    let wait_semaphores = [vk::SemaphoreSubmitInfo {
      s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
      p_next: ptr::null(),
      semaphore: self.image_available[cur_frame_i],
      value: 0,                                                     // ignored
      stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT, // todo: explain why
      device_index: 0,                                              // ignored
      _marker: PhantomData,
    }];
    let signal_semaphores = [
      vk::SemaphoreSubmitInfo {
        s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
        p_next: ptr::null(),
        semaphore: self.presentable[cur_frame_i],
        value: 0,
        stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
        device_index: 0, // ignored
        _marker: PhantomData,
      },
      vk::SemaphoreSubmitInfo {
        s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
        p_next: ptr::null(),
        semaphore: self.timeline,
        value: next_timeline_value,
        stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
        device_index: 0, // ignored
        _marker: PhantomData,
      },
    ];
    let submit_info = vk::SubmitInfo2::default()
      .command_buffer_infos(&command_buffers)
      .wait_semaphore_infos(&wait_semaphores)
      .signal_semaphore_infos(&signal_semaphores);
    unsafe {
      self
        .renderer
        .device
        .queue_submit2(
          self.renderer.queues.graphics,
          &[submit_info],
          vk::Fence::null(),
        )
        .unwrap();
    }
    self.timeline_index = next_timeline_value;

    unsafe {
      if let Err(err) = self.renderer.swapchains.queue_present(
        image_index,
        self.renderer.queues.presentation,
        &[self.presentable[cur_frame_i]],
      ) {
        self.recreate_swapchain_next_frame = true;
        return Err(err.into());
      }
    }

    Ok(())
  }
}

impl Drop for SyncRenderer {
  fn drop(&mut self) {
    unsafe {
      self
        .renderer
        .device
        .device_wait_idle()
        .expect("Failed to wait for device idleness while dropping SyncRenderer");

      self.timeline.destroy_self(&self.renderer.device);
      for sem in self.image_available {
        sem.destroy_self(&self.renderer.device);
      }
      for sem in self.presentable {
        sem.destroy_self(&self.renderer.device);
      }
    }
  }
}
