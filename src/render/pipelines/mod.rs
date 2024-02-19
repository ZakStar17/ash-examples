mod cache;
mod compute;
mod graphics;

use ash::vk::{self, PipelineCache};

pub use self::{compute::ComputePipelines, graphics::GraphicsPipelines};

use super::{descriptor_sets::DescriptorSets, initialization::PhysicalDevice};

pub struct Pipelines {
  pub compute: ComputePipelines,
  pub graphics: GraphicsPipelines,
  cache: PipelineCache,
}

impl Pipelines {
  pub fn new(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    descriptor_sets: &DescriptorSets,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
  ) -> Self {
    log::info!("Creating pipeline cache");
    let (pipeline_cache, created_from_file) =
      cache::create_pipeline_cache(&device, &physical_device);
    if created_from_file {
      log::info!("Cache successfully created from an existing cache file");
    } else {
      log::info!("Cache initialized as empty");
    }

    Self {
      graphics: GraphicsPipelines::new(
        device,
        pipeline_cache,
        descriptor_sets,
        render_pass,
        extent,
      ),
      compute: ComputePipelines::new(device, pipeline_cache, descriptor_sets),
      cache: pipeline_cache,
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device, physical_device: &PhysicalDevice) {
    self.graphics.destroy_self(device);
    self.compute.destroy_self(device);

    log::info!("Saving pipeline cache");
    if let Err(err) = cache::save_pipeline_cache(device, physical_device, self.cache) {
      log::error!("Failed to save pipeline cache: {:?}", err);
    }
    device.destroy_pipeline_cache(self.cache, None);
  }
}
