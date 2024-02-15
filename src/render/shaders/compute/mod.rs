use std::{ffi::CStr, path::Path, ptr};

use ash::vk;

use crate::utility::cstr;

use super::load_shader;

const PATH: &'static str = "./shaders/compute/shader.spv";

const MAIN_FN_NAME: &'static CStr = cstr!("main");

pub struct Shader {
  pub module: vk::ShaderModule,
}

impl Shader {
  pub fn load(device: &ash::Device) -> Self {
    Self {
      module: load_shader(device, Path::new(PATH)),
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_shader_module(self.module, None);
  }
}

impl Shader {
  pub fn get_pipeline_shader_creation_info(&self) -> vk::PipelineShaderStageCreateInfo {
    vk::PipelineShaderStageCreateInfo {
      s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineShaderStageCreateFlags::empty(),
      module: self.module,
      p_name: MAIN_FN_NAME.as_ptr(),
      p_specialization_info: ptr::null(),
      stage: vk::ShaderStageFlags::COMPUTE,
    }
  }
}
