use std::ops::BitOr;

use ash::vk;

use crate::{
  render::{
    create_objs::{create_image, create_image_view},
    render_pass::create_framebuffer,
    RENDER_EXTENT,
  },
  utility::OnErr,
};

use super::{
  allocator::allocate_and_bind_memory,
  device_destroyable::{
    destroy, fill_destroyable_array_from_iter, fill_destroyable_array_with_expression,
    DeviceManuallyDestroyed,
  },
  errors::AllocationError,
  initialization::device::{Device, PhysicalDevice},
  FRAMES_IN_FLIGHT,
};

// images that the main graphics pipeline draws to
// these are then copied to the swapchain image
#[derive(Debug)]
pub struct RenderTargets {
  pub images: [vk::Image; FRAMES_IN_FLIGHT],
  pub memory: vk::DeviceMemory,
  pub image_views: [vk::ImageView; FRAMES_IN_FLIGHT],
  pub framebuffers: [vk::Framebuffer; FRAMES_IN_FLIGHT],
}

impl RenderTargets {
  const PRIORITY: f32 = 0.8; // high priority

  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    render_pass: vk::RenderPass,
    render_format: vk::Format,
  ) -> Result<Self, AllocationError> {
    let images = fill_destroyable_array_with_expression!(
      device,
      create_image(
        device,
        render_format,
        RENDER_EXTENT.width,
        RENDER_EXTENT.height,
        vk::ImageUsageFlags::COLOR_ATTACHMENT
          .bitor(vk::ImageUsageFlags::TRANSFER_SRC)
          .bitor(vk::ImageUsageFlags::TRANSFER_DST)
      ),
      FRAMES_IN_FLIGHT
    )?;

    let memory_requirements =
      images.map(|image| unsafe { device.get_image_memory_requirements(image) });
    let memory = {
      let allocation = allocate_and_bind_memory(
        device,
        physical_device,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        &[],
        &[],
        &images,
        &memory_requirements,
        Self::PRIORITY,
      )
      .on_err(|_| unsafe { images.destroy_self(device) })?;
      allocation.memory
    };

    let image_views = fill_destroyable_array_from_iter!(
      device,
      images
        .iter()
        .map(|image| create_image_view(device, *image, render_format)),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { destroy!(device => images.as_ref(), &memory) })?;

    let framebuffers = fill_destroyable_array_from_iter!(
      device,
      image_views
        .iter()
        .map(|view| create_framebuffer(device, render_pass, *view, RENDER_EXTENT)),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { destroy!(device => image_views.as_ref(), images.as_ref(), &memory) })?;

    Ok(Self {
      images,
      image_views,
      memory,
      framebuffers,
    })
  }
}

impl DeviceManuallyDestroyed for RenderTargets {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.framebuffers.destroy_self(device);
    self.image_views.destroy_self(device);
    self.images.destroy_self(device);
    self.memory.destroy_self(device);
  }
}
