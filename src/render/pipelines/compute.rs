use std::{mem::size_of, ptr};

use ash::vk;

use crate::render::{compute_data::ComputePushConstants, descriptor_sets::DescriptorSets, shaders};


pub struct ComputePipelines {
  pub layout: vk::PipelineLayout,
  pub compute_instances: vk::Pipeline,
}

impl ComputePipelines {
  pub fn new(
    device: &ash::Device,
    cache: vk::PipelineCache,
    descriptor_sets: &DescriptorSets,
  ) -> Self {
    let mut shader = shaders::compute::Shader::load(device);

    let push_constant_range = vk::PushConstantRange {
      stage_flags: vk::ShaderStageFlags::COMPUTE,
      offset: 0,
      size: size_of::<ComputePushConstants>() as u32,
    };
    let layout_create_info = vk::PipelineLayoutCreateInfo {
      s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineLayoutCreateFlags::empty(),
      set_layout_count: 1,
      p_set_layouts: &descriptor_sets.compute_layout,
      push_constant_range_count: 1,
      p_push_constant_ranges: &push_constant_range,
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
    let compute_instances = unsafe {
      device
        .create_compute_pipelines(cache, &[create_info], None)
        .expect("Failed to create compute pipelines")[0]
    };

    unsafe {
      shader.destroy_self(device);
    }

    Self { layout, compute_instances }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_pipeline(self.compute_instances, None);
    device.destroy_pipeline_layout(self.layout, None);
  }
}
