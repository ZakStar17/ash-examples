use std::{
  ffi::CString,
  ptr::{self, addr_of},
};

use ash::vk;

use crate::{descriptor_sets::DescriptorSets, shaders::Shader};

pub struct ComputePipeline {
  pub layout: vk::PipelineLayout,
  pub pipeline: vk::Pipeline,
}

impl ComputePipeline {
  pub fn create(device: &ash::Device, descriptor_sets: &DescriptorSets) -> Self {
    let mut shader = Shader::load(device);
    let main_function_name = CString::new("main").unwrap(); // the beginning function name in shader code

    let stage = vk::PipelineShaderStageCreateInfo {
      s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineShaderStageCreateFlags::empty(),
      module: shader.module,
      p_name: main_function_name.as_ptr(),
      p_specialization_info: ptr::null(),
      stage: vk::ShaderStageFlags::COMPUTE,
    };
    let layout_create_info = vk::PipelineLayoutCreateInfo {
      s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineLayoutCreateFlags::empty(),
      set_layout_count: 1,
      p_set_layouts: addr_of!(descriptor_sets.layout),
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
      stage,
      flags: vk::PipelineCreateFlags::empty(),
      layout,
      base_pipeline_handle: vk::Pipeline::null(),
      base_pipeline_index: -1, // -1 for invalid
    };

    let pipeline = unsafe {
      device
        .create_compute_pipelines(vk::PipelineCache::null(), &[create_info], None)
        .expect("Failed to create compute pipelines")[0]
    };

    unsafe {
      shader.destroy_self(device);
    }

    Self {
      layout,
      pipeline: pipeline,
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_pipeline(self.pipeline, None);
    device.destroy_pipeline_layout(self.layout, None);
  }
}
