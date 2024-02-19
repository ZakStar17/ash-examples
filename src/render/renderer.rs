use std::ops::BitOr;

use ash::vk;
use image::ImageError;
use winit::dpi::PhysicalSize;

use crate::{
  render::{
    allocations::allocate_and_bind_memory,
    command_pools::{TemporaryGraphicsCommandPool, TransferCommandPool},
    common_object_creations::{create_image, create_image_view},
    initialization::device::create_logical_device,
    long_lived::{create_framebuffer, create_render_pass},
    RENDER_FORMAT, TEXTURE_PATH,
  },
  utility::{self, populate_array_with_expression},
  RESOLUTION,
};

use super::{
  command_pools::{
    compute::{ComputeCommandPool, ComputeRecordBufferData},
    GraphicsCommandPool,
  },
  compute_data::ComputeData,
  constant_data::ConstantData,
  descriptor_sets::DescriptorSets,
  initialization::{PhysicalDevice, Queues, Surface},
  long_lived::Swapchains,
  pipelines::Pipelines,
  push_constants::SpritePushConstants,
  FRAMES_IN_FLIGHT,
};

const RENDER_EXTENT: vk::Extent2D = vk::Extent2D {
  width: RESOLUTION[0],
  height: RESOLUTION[1],
};

fn read_texture_bytes_as_rgba8() -> Result<(u32, u32, Vec<u8>), ImageError> {
  let img = image::io::Reader::open(TEXTURE_PATH)?
    .decode()?
    .into_rgba8();
  let width = img.width();
  let height = img.height();

  let bytes = img.into_raw();
  assert!(bytes.len() == width as usize * height as usize * 4);
  Ok((width, height, bytes))
}

pub struct Renderer {
  pub physical_device: PhysicalDevice,
  pub device: ash::Device,
  pub queues: Queues,

  pub swapchains: Swapchains,

  render_pass: vk::RenderPass,
  // images to be rendered to
  render_targets: [vk::Image; FRAMES_IN_FLIGHT],
  render_targets_memory: vk::DeviceMemory,
  render_target_views: [vk::ImageView; FRAMES_IN_FLIGHT],
  framebuffers: [vk::Framebuffer; FRAMES_IN_FLIGHT],

  descriptor_sets: DescriptorSets,
  pipelines: Pipelines,

  pub graphics_pools: [GraphicsCommandPool; FRAMES_IN_FLIGHT],
  pub compute_pools: [ComputeCommandPool; FRAMES_IN_FLIGHT],

  constant_data: ConstantData,
  pub compute_data: ComputeData,
}

impl Renderer {
  pub fn new(
    instance: &ash::Instance,
    surface: &Surface,
    initial_window_size: PhysicalSize<u32>,
  ) -> Self {
    let physical_device = unsafe { PhysicalDevice::select(&instance, surface) };
    let (device, queues) = create_logical_device(&instance, &physical_device);
    log::debug!("Queue handles:\n{:#?}", queues);

    let render_pass = create_render_pass(&device, RENDER_FORMAT);
    let render_targets = utility::populate_array_with_expression!(
      create_image(
        &device,
        RESOLUTION[0],
        RESOLUTION[1],
        RENDER_FORMAT,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::COLOR_ATTACHMENT.bitor(vk::ImageUsageFlags::TRANSFER_SRC)
      ),
      2
    );
    let render_targets_memory = {
      let allocation = allocate_and_bind_memory(
        &device,
        &physical_device,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        vk::MemoryPropertyFlags::empty(),
        &[],
        &render_targets,
      )
      .expect("Failed to allocate main render target images");
      allocation.memory
    };
    let render_target_views = {
      let mut temp = [vk::ImageView::null(); 2];
      temp[0] = create_image_view(&device, render_targets[0], RENDER_FORMAT);
      temp[1] = create_image_view(&device, render_targets[1], RENDER_FORMAT);
      temp
    };
    let framebuffers = {
      let mut temp = [vk::Framebuffer::null(); 2];
      temp[0] = create_framebuffer(&device, render_pass, render_target_views[0], RENDER_EXTENT);
      temp[1] = create_framebuffer(&device, render_pass, render_target_views[1], RENDER_EXTENT);
      temp
    };

    let mut descriptor_sets = DescriptorSets::new(&device);

    let pipelines = Pipelines::new(
      &device,
      &physical_device,
      &descriptor_sets,
      render_pass,
      RENDER_EXTENT,
    );

    let constant_data = {
      let mut transfer_pool = TransferCommandPool::create(&device, &physical_device.queue_families);
      let mut graphics_pool =
        TemporaryGraphicsCommandPool::create(&device, &physical_device.queue_families);

      let (texture_width, texture_height, texture_bytes) =
        read_texture_bytes_as_rgba8().expect("Failed to read texture file");

      let objects = ConstantData::new(
        &device,
        &physical_device,
        &queues,
        &mut transfer_pool,
        &mut graphics_pool,
        &texture_bytes,
        texture_width,
        texture_height,
      );

      unsafe {
        transfer_pool.destroy_self(&device);
        graphics_pool.destroy_self(&device);
      }

      objects
    };

    let compute_data = ComputeData::new(&device, &physical_device);

    descriptor_sets.write_sets(&device, constant_data.texture_view, &compute_data);

    let graphics_pools = populate_array_with_expression!(
      GraphicsCommandPool::create(&device, &physical_device.queue_families),
      FRAMES_IN_FLIGHT
    );
    let compute_pools = populate_array_with_expression!(
      ComputeCommandPool::create(&device, &physical_device.queue_families),
      FRAMES_IN_FLIGHT
    );

    let swapchains = Swapchains::new(
      instance,
      &physical_device,
      &device,
      surface,
      initial_window_size,
    );

    Self {
      physical_device,
      device,
      queues,

      swapchains,

      render_pass,
      render_targets,
      render_targets_memory,
      render_target_views,
      framebuffers,

      descriptor_sets,
      pipelines,

      graphics_pools,
      compute_pools,

      constant_data,
      compute_data,
    }
  }

  pub unsafe fn record_graphics(
    &mut self,
    frame_i: usize,
    image_i: usize,
    bullet_instance_count: u32,
    player: &SpritePushConstants,
  ) {
    self.graphics_pools[frame_i].record(
      &self.device,
      &self.physical_device.queue_families,
      self.render_pass,
      self.render_targets[frame_i],
      self.framebuffers[frame_i],
      RENDER_EXTENT,
      self.swapchains.get_images()[image_i],
      self.swapchains.get_extent(),
      &self.descriptor_sets,
      &self.pipelines.graphics,
      &self.constant_data,
      self.compute_data.instance_graphics[frame_i],
      bullet_instance_count,
      player,
    );
  }

  pub unsafe fn record_compute(&mut self, frame_i: usize, data: ComputeRecordBufferData) {
    self.compute_pools[frame_i].record(
      &self.device,
      &self.physical_device.queue_families,
      &self.pipelines.compute,
      self.descriptor_sets.compute_sets[frame_i],
      data,
    )
  }

  pub unsafe fn recreate_swapchain(&mut self, surface: &Surface, window_size: PhysicalSize<u32>) {
    // old swapchain becomes retired
    let changes =
      self
        .swapchains
        .recreate_swapchain(&self.physical_device, &self.device, surface, window_size);

    if !changes.format && !changes.extent {
      log::warn!("Recreated swapchain without any extent or format change");
    }
  }

  // destroy old objects that resulted of a swapchain recreation
  // this should only be called when they stop being in use
  pub unsafe fn destroy_old(&mut self) {
    self.swapchains.destroy_old(&self.device);
  }

  pub unsafe fn destroy_self(&mut self) {
    self.compute_data.destroy_self(&self.device);
    self.constant_data.destroy_self(&self.device);

    for pool in self.graphics_pools.iter_mut() {
      pool.destroy_self(&self.device);
    }
    for pool in self.compute_pools.iter_mut() {
      pool.destroy_self(&self.device);
    }

    self
      .pipelines
      .destroy_self(&self.device, &self.physical_device);
    self.descriptor_sets.destroy_self(&self.device);

    for &framebuffer in self.framebuffers.iter() {
      self.device.destroy_framebuffer(framebuffer, None);
    }
    for &view in self.render_target_views.iter() {
      self.device.destroy_image_view(view, None);
    }
    for &image in self.render_targets.iter() {
      self.device.destroy_image(image, None);
    }
    self.device.free_memory(self.render_targets_memory, None);

    self.device.destroy_render_pass(self.render_pass, None);

    self.swapchains.destroy_self(&self.device);

    self.device.destroy_device(None);
  }
}
