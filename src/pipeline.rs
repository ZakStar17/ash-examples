use std::{
  ffi::{c_void, CString}, mem::{offset_of, size_of}, ptr::{self, addr_of}
};

use ash::vk;

use crate::{descriptor_sets::DescriptorSets, shaders::{shader::SHADER_GROUP_SIZE_X, Shader}};

pub struct ComputePipeline {
  pub layout: vk::PipelineLayout,
  pub pipeline: vk::Pipeline,
}

#[repr(C)]
struct SpecializationData {
  group_size: u32,
  max_iterations: u32,
  focal_point_x: f32,
  focal_point_y: f32,
  zoom: f32,
}

impl SpecializationData {
  const fn entries() -> [vk::SpecializationMapEntry; 5] {
    [
      vk::SpecializationMapEntry {
        constant_id: 0,
        offset: offset_of!(SpecializationData, group_size) as u32,
        size: size_of::<u32>()
      },
      vk::SpecializationMapEntry {
        constant_id: 1,
        offset: offset_of!(SpecializationData, max_iterations) as u32,
        size: size_of::<u32>()
      },
      vk::SpecializationMapEntry {
        constant_id: 2,
        offset: offset_of!(SpecializationData, focal_point_x) as u32,
        size: size_of::<f32>()
      },
      vk::SpecializationMapEntry {
        constant_id: 3,
        offset: offset_of!(SpecializationData, focal_point_y) as u32,
        size: size_of::<f32>()
      },
      vk::SpecializationMapEntry {
        constant_id: 4,
        offset: offset_of!(SpecializationData, zoom) as u32,
        size: size_of::<f32>()
      },
    ]
  }
}


impl ComputePipeline {
  pub fn create(device: &ash::Device, descriptor_sets: &DescriptorSets) -> Self {
    let mut shader = Shader::load(device);
    let main_function_name = CString::new("main").unwrap(); // the beginning function name in shader code

    let specialization_data = SpecializationData {
      group_size: SHADER_GROUP_SIZE_X,
      max_iterations: 10000,
      focal_point_x: -0.765,
      focal_point_y: 0.0,
      zoom: 0.40486
    };
    let entries = SpecializationData::entries();
    let specialization_info = vk::SpecializationInfo {
      map_entry_count: entries.len() as u32,
      p_map_entries: entries.as_ptr(),
      data_size: size_of::<SpecializationData>(),
      p_data: addr_of!(specialization_data) as *const c_void,
    };

    let stage = vk::PipelineShaderStageCreateInfo {
      s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineShaderStageCreateFlags::empty(),
      module: shader.module,
      p_name: main_function_name.as_ptr(),
      p_specialization_info: addr_of!(specialization_info),
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
      pipeline,
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_pipeline(self.pipeline, None);
    device.destroy_pipeline_layout(self.layout, None);
  }
}
