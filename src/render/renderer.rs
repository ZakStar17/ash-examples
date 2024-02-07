use ash::vk;
use winit::dpi::PhysicalSize;

use crate::{
  render::objects::{
    command_pools::{GraphicsCommandBufferPool, TransferCommandBufferPool},
    create_pipeline_cache, ConstantBuffers,
  },
  utility::populate_array_with_expression,
};

use super::{
  objects::{
    create_framebuffer, create_render_pass,
    device::{create_logical_device, PhysicalDevice, Queues},
    save_pipeline_cache, GraphicsPipeline, Surface, Swapchains,
  },
  RenderPosition, FRAMES_IN_FLIGHT,
};

pub struct Renderer {
  pub physical_device: PhysicalDevice,
  pub device: ash::Device,
  pub queues: Queues,

  pub swapchains: Swapchains,
  render_pass: vk::RenderPass,
  framebuffers: Box<[vk::Framebuffer]>,
  old_framebuffers: Option<Box<[vk::Framebuffer]>>,

  pipeline_cache: vk::PipelineCache,
  pipeline: GraphicsPipeline,

  pub graphics_pools: [GraphicsCommandBufferPool; FRAMES_IN_FLIGHT],
  buffers: ConstantBuffers,
}

impl Renderer {
  pub fn new(
    instance: &ash::Instance,
    surface: &Surface,
    initial_window_size: PhysicalSize<u32>,
  ) -> Self {
    let physical_device = unsafe { PhysicalDevice::select(&instance, surface) };
    let (device, queues) = create_logical_device(&instance, &physical_device);

    let swapchains = Swapchains::new(
      instance,
      *physical_device,
      &device,
      surface,
      initial_window_size,
    );

    let render_pass = create_render_pass(&device, swapchains.get_format());
    let framebuffers = swapchains
      .get_image_views()
      .iter()
      .map(|&view| create_framebuffer(&device, render_pass, view, swapchains.get_extent()))
      .collect();

    log::info!("Creating pipeline cache");
    let (pipeline_cache, created_from_file) = create_pipeline_cache(&device, &physical_device);
    if created_from_file {
      log::info!("Cache successfully created from an existing cache file");
    } else {
      log::info!("Cache initialized as empty");
    }
    let pipeline = GraphicsPipeline::create(
      &device,
      pipeline_cache,
      render_pass,
      swapchains.get_extent(),
    );

    let mut transfer_pool =
      TransferCommandBufferPool::create(&device, &physical_device.queue_families);
    let buffers = ConstantBuffers::new(&device, &physical_device, &queues, &mut transfer_pool);
    unsafe {
      transfer_pool.destroy_self(&device);
    }

    let graphics_pools = populate_array_with_expression!(
      GraphicsCommandBufferPool::create(&device, &physical_device.queue_families),
      FRAMES_IN_FLIGHT
    );

    Self {
      physical_device,
      device,
      queues,

      swapchains,
      render_pass,
      framebuffers,
      old_framebuffers: None,

      pipeline_cache,
      pipeline,

      graphics_pools,
      buffers,
    }
  }

  pub unsafe fn record_graphics(
    &mut self,
    frame_i: usize,
    image_i: usize,
    position: &RenderPosition,
  ) {
    self.graphics_pools[frame_i].record(
      &self.device,
      &self.physical_device.queue_families,
      self.render_pass,
      self.swapchains.get_extent(),
      self.framebuffers[image_i],
      &self.pipeline,
      &self.buffers,
      self.swapchains.get_images()[image_i],
      position,
    );
  }

  pub unsafe fn recreate_swapchain(&mut self, surface: &Surface, window_size: PhysicalSize<u32>) {
    // it is possible to use more than two frames in flight, but it would require having more than one old swapchain and pipeline
    assert!(FRAMES_IN_FLIGHT == 2);

    // this function shouldn't be called if old objects haven't been destroyed
    assert!(self.old_framebuffers.is_none());

    // old swapchain becomes retired
    let changes =
      self
        .swapchains
        .recreate_swapchain(*self.physical_device, &self.device, surface, window_size);

    if changes.format {
      log::info!("Changing swapchain format");

      // this shouldn't happen regularly, so its okay to stop all rendering so that the render pass can be recreated
      self
        .device
        .device_wait_idle()
        .expect("Failed to wait for device idleness while recreating swapchain and format");

      self.device.destroy_render_pass(self.render_pass, None);
      self.render_pass = create_render_pass(&self.device, self.swapchains.get_format());
    } else {
      if !changes.extent {
        log::warn!("Recreating swapchain without any extent or format change");
      }
    }

    let mut new_framebuffers = self
      .swapchains
      .get_image_views()
      .iter()
      .map(|&view| {
        create_framebuffer(
          &self.device,
          self.render_pass,
          view,
          self.swapchains.get_extent(),
        )
      })
      .collect();

    let old_framebuffers = {
      std::mem::swap(&mut self.framebuffers, &mut new_framebuffers);
      new_framebuffers
    };
    self.old_framebuffers = Some(old_framebuffers);

    self.pipeline.recreate(
      &self.device,
      self.pipeline_cache,
      self.render_pass,
      self.swapchains.get_extent(),
    );
  }

  // destroy old objects that resulted of a swapchain recreation
  // this should only be called when they stop being in use
  pub unsafe fn destroy_old(&mut self) {
    self.pipeline.destroy_old(&self.device);

    for &framebuffer in self.old_framebuffers.as_mut().unwrap().iter() {
      self.device.destroy_framebuffer(framebuffer, None);
    }
    self.old_framebuffers = None;

    self.swapchains.destroy_old(&self.device);
  }

  pub unsafe fn destroy_self(&mut self) {
    self.buffers.destroy_self(&self.device);
    for pool in self.graphics_pools.iter_mut() {
      pool.destroy_self(&self.device);
    }

    log::info!("Saving pipeline cache");
    if let Err(err) = save_pipeline_cache(&self.device, &self.physical_device, self.pipeline_cache)
    {
      log::error!("Failed to save pipeline cache: {:?}", err);
    }
    self
      .device
      .destroy_pipeline_cache(self.pipeline_cache, None);

    self.pipeline.destroy_self(&self.device);

    for &framebuffer in self.framebuffers.iter() {
      self.device.destroy_framebuffer(framebuffer, None);
    }
    if let Some(old) = self.old_framebuffers.as_mut() {
      for &framebuffer in old.iter() {
        self.device.destroy_framebuffer(framebuffer, None);
      }
    }

    self.device.destroy_render_pass(self.render_pass, None);

    self.swapchains.destroy_self(&self.device);

    self.device.destroy_device(None);
  }
}
