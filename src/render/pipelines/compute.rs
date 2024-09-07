use std::{marker::PhantomData, mem::size_of, ptr};

use ash::vk;

use crate::render::{
  data::compute::ComputePushConstants, descriptor_sets::DescriptorPool,
  device_destroyable::DeviceManuallyDestroyed, errors::OutOfMemoryError, shaders,
};

use super::PipelineCreationError;

#[derive(Debug)]
pub struct ComputePipelines {
  pub layout: vk::PipelineLayout,
  pub instance: vk::Pipeline,
}

impl ComputePipelines {
  pub fn new(
    device: &ash::Device,
    cache: vk::PipelineCache,
    descriptor_pool: &DescriptorPool,
  ) -> Result<Self, PipelineCreationError> {
    let layout = Self::create_layout(device, descriptor_pool)?;

    let shader =
      shaders::compute::Shader::load(device).map_err(PipelineCreationError::ShaderFailed)?;
    let shader_stages = shader.get_pipeline_shader_creation_info();

    let create_info = vk::ComputePipelineCreateInfo {
      s_type: vk::StructureType::COMPUTE_PIPELINE_CREATE_INFO,
      p_next: ptr::null(),
      stage: shader_stages,
      flags: vk::PipelineCreateFlags::empty(),
      layout,
      base_pipeline_handle: vk::Pipeline::null(),
      base_pipeline_index: -1, // -1 for invalid
      _marker: PhantomData,
    };
    let instance = unsafe { device.create_compute_pipelines(cache, &[create_info], None) }
      .map_err(|incomplete| incomplete.1)
      .map_err(|vkerr| match vkerr {
        vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
          PipelineCreationError::from(OutOfMemoryError::from(vkerr))
        }
        vk::Result::ERROR_INVALID_SHADER_NV => PipelineCreationError::CompilationFailed,
        _ => panic!(),
      })?[0];

    unsafe {
      shader.destroy_self(device);
    }

    Ok(Self { layout, instance })
  }

  fn create_layout(
    device: &ash::Device,
    descriptor_pool: &DescriptorPool,
  ) -> Result<vk::PipelineLayout, OutOfMemoryError> {
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
      p_set_layouts: &descriptor_pool.compute_layout,
      push_constant_range_count: 1,
      p_push_constant_ranges: &push_constant_range,
      _marker: PhantomData,
    };
    unsafe { device.create_pipeline_layout(&layout_create_info, None) }
      .map_err(OutOfMemoryError::from)
  }
}

impl DeviceManuallyDestroyed for ComputePipelines {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.instance.destroy_self(device);
    self.layout.destroy_self(device);
  }
}
