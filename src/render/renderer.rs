use ash::vk;
use winit::dpi::PhysicalSize;

use crate::render::objects::create_pipeline_cache;

use super::objects::{
  create_framebuffer, create_render_pass, device::{create_logical_device, PhysicalDevice}, save_pipeline_cache, GraphicsPipeline, Surface, Swapchains
};

pub struct Renderer {
  physical_device: PhysicalDevice,
  device: ash::Device,

  swapchains: Swapchains,
  render_pass: vk::RenderPass,
  framebuffers: Box<[vk::Framebuffer]>,

  pipeline_cache: vk::PipelineCache,
  pipeline: GraphicsPipeline,
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
      &physical_device,
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

    Self {
      physical_device,
      device,

      swapchains,
      render_pass,
      framebuffers,

      pipeline_cache,
      pipeline
    }
  }

  pub unsafe fn destroy_self(&mut self) {
    log::info!("Saving pipeline cache");
    if let Err(err) = save_pipeline_cache(&self.device, &self.physical_device, self.pipeline_cache) {
      log::error!("Failed to save pipeline cache: {:?}", err);
    }
    self.device.destroy_pipeline_cache(self.pipeline_cache, None);
    
    self.pipeline.destroy_self(&self.device);

    for &framebuffer in self.framebuffers.iter() {
      self.device.destroy_framebuffer(framebuffer, None);
    }
    self.device.destroy_render_pass(self.render_pass, None);

    self.swapchains.destroy_old(&self.device);
    self.swapchains.destroy_self(&self.device);

    self.device.destroy_device(None);
  }
}
