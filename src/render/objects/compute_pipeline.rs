use std::ptr::{self};

use ash::vk;

use crate::render::shaders;

use super::DescriptorSets;

pub struct ComputePipeline {
  pub layout: vk::PipelineLayout,
  pub pipeline: vk::Pipeline,
}

impl ComputePipeline {
  pub fn new(
    device: &ash::Device,
    cache: vk::PipelineCache,
    descriptor_sets: &DescriptorSets,
  ) -> Self {
    let mut shader = shaders::compute::Shader::load(device);

    let layout_create_info = vk::PipelineLayoutCreateInfo {
      s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineLayoutCreateFlags::empty(),
      set_layout_count: 1,
      p_set_layouts: &descriptor_sets.compute_layout,
      push_constant_range_count: 0,
      p_push_constant_ranges: ptr::null(),
    };
    let layout = unsafe {
      device
        .create_pipeline_layout(&layout_create_info, None)
        .expect("Failed to create pipeline layout")
    };

    let create_info = vk::ComputePipelineCreateInfo {
      s_type: vk::StructureType::COMPUTE_PIPELINE_CREATE_INFO,
      p_next: ptr::null(),
      stage: shader.get_pipeline_shader_creation_info(),
      flags: vk::PipelineCreateFlags::empty(),
      layout,
      base_pipeline_handle: vk::Pipeline::null(),
      base_pipeline_index: -1, // -1 for invalid
    };
    let pipeline = unsafe {
      device
        .create_compute_pipelines(cache, &[create_info], None)
        .expect("Failed to create compute pipelines")[0]
    };

    unsafe {
      shader.destroy_self(device);
    }

    Self { layout, pipeline }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_pipeline(self.pipeline, None);
    device.destroy_pipeline_layout(self.layout, None);
  }
}
