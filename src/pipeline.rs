use std::{
  ffi::{c_void, CString},
  marker::PhantomData,
  mem::{offset_of, size_of},
  ptr::{self, addr_of},
};

use ash::vk;

use crate::{
  descriptor_sets::DescriptorSets,
  device_destroyable::DeviceManuallyDestroyed,
  errors::OutOfMemoryError,
  shaders::{shader, Shader, ShaderError},
  FOCAL_POINT, MAX_ITERATIONS, SHADER_GROUP_SIZE_X, SHADER_GROUP_SIZE_Y, ZOOM,
};

pub struct ComputePipeline {
  pub layout: vk::PipelineLayout,
  pub pipeline: vk::Pipeline,
}

#[repr(C)]
struct SpecializationData {
  group_size_x: u32,
  group_size_y: u32,
  max_iterations: u32,
  focal_point_x: f32,
  focal_point_y: f32,
  zoom: f32,
}

impl SpecializationData {
  const fn entries() -> [vk::SpecializationMapEntry; 6] {
    [
      vk::SpecializationMapEntry {
        constant_id: 0,
        offset: offset_of!(SpecializationData, group_size_x) as u32,
        size: size_of::<u32>(),
      },
      vk::SpecializationMapEntry {
        constant_id: 1,
        offset: offset_of!(SpecializationData, group_size_y) as u32,
        size: size_of::<u32>(),
      },
      vk::SpecializationMapEntry {
        constant_id: 2,
        offset: offset_of!(SpecializationData, max_iterations) as u32,
        size: size_of::<u32>(),
      },
      vk::SpecializationMapEntry {
        constant_id: 3,
        offset: offset_of!(SpecializationData, focal_point_x) as u32,
        size: size_of::<f32>(),
      },
      vk::SpecializationMapEntry {
        constant_id: 4,
        offset: offset_of!(SpecializationData, focal_point_y) as u32,
        size: size_of::<f32>(),
      },
      vk::SpecializationMapEntry {
        constant_id: 5,
        offset: offset_of!(SpecializationData, zoom) as u32,
        size: size_of::<f32>(),
      },
    ]
  }
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineCreationError {
  #[error("Out of memory")]
  OutOfMemory(#[source] OutOfMemoryError),
  #[error("Failed to load shader \"{1}\"")]
  ShaderFailed(#[source] ShaderError, &'static str),
  #[error("Failed to compile or link shaders")]
  CompilationFailed,
}

impl From<OutOfMemoryError> for PipelineCreationError {
  fn from(value: OutOfMemoryError) -> Self {
    PipelineCreationError::OutOfMemory(value)
  }
}

impl ComputePipeline {
  pub fn create(
    device: &ash::Device,
    cache: vk::PipelineCache,
    descriptor_sets: &DescriptorSets,
  ) -> Result<Self, PipelineCreationError> {
    let mut shader = Shader::load(device)
      .map_err(|err| PipelineCreationError::ShaderFailed(err, shader::SHADER_PATH))?;
    let main_function_name = CString::new("main").unwrap(); // the beginning function name in shader code

    let specialization_data = SpecializationData {
      group_size_x: SHADER_GROUP_SIZE_X,
      group_size_y: SHADER_GROUP_SIZE_Y,
      max_iterations: MAX_ITERATIONS,
      focal_point_x: FOCAL_POINT[0],
      focal_point_y: FOCAL_POINT[1],
      zoom: ZOOM,
    };
    let entries = SpecializationData::entries();
    let specialization_info = vk::SpecializationInfo {
      map_entry_count: entries.len() as u32,
      p_map_entries: entries.as_ptr(),
      data_size: size_of::<SpecializationData>(),
      p_data: addr_of!(specialization_data) as *const c_void,
      _marker: PhantomData,
    };

    let stage = vk::PipelineShaderStageCreateInfo {
      s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineShaderStageCreateFlags::empty(),
      module: shader.module,
      p_name: main_function_name.as_ptr(),
      p_specialization_info: addr_of!(specialization_info),
      stage: vk::ShaderStageFlags::COMPUTE,
      _marker: PhantomData,
    };

    let layout_create_info = vk::PipelineLayoutCreateInfo {
      s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineLayoutCreateFlags::empty(),
      set_layout_count: 1,
      p_set_layouts: addr_of!(descriptor_sets.layout),
      push_constant_range_count: 0,
      p_push_constant_ranges: ptr::null(),
      _marker: PhantomData,
    };
    let layout = unsafe { device.create_pipeline_layout(&layout_create_info, None) }
      .map_err(|vkerr| OutOfMemoryError::from(vkerr))?;

    let create_info = vk::ComputePipelineCreateInfo {
      s_type: vk::StructureType::COMPUTE_PIPELINE_CREATE_INFO,
      p_next: ptr::null(),
      stage,
      flags: vk::PipelineCreateFlags::empty(),
      layout,
      base_pipeline_handle: vk::Pipeline::null(),
      base_pipeline_index: -1, // -1 for invalid
      _marker: PhantomData,
    };

    let pipeline = unsafe {
      device
        .create_compute_pipelines(cache, &[create_info], None)
        .map_err(|incomplete| incomplete.1)
        .map_err(|vkerr| match vkerr {
          vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
            PipelineCreationError::from(OutOfMemoryError::from(vkerr))
          }
          vk::Result::ERROR_INVALID_SHADER_NV => PipelineCreationError::CompilationFailed,
          _ => panic!(),
        })?[0]
    };

    unsafe {
      shader.destroy_self(device);
    }

    Ok(Self { layout, pipeline })
  }
}

impl DeviceManuallyDestroyed for ComputePipeline {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_pipeline(self.pipeline, None);
    device.destroy_pipeline_layout(self.layout, None);
  }
}
