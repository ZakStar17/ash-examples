use std::{ffi::CStr, path::Path, ptr};

use ash::vk;

use crate::utility::cstr;

use super::load_shader;

const VERT_SHADER_PATH: &'static str = "./shaders/vert.spv";
const FRAG_SHADER_PATH: &'static str = "./shaders/frag.spv";

const MAIN_FN_NAME: &'static CStr = cstr!("main");

pub struct Shader {
  pub vert: vk::ShaderModule,
  pub frag: vk::ShaderModule,
}

impl Shader {
  pub fn load(device: &ash::Device) -> Self {
    Self {
      vert: load_shader(device, Path::new(VERT_SHADER_PATH)),
      frag: load_shader(device, Path::new(FRAG_SHADER_PATH)),
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_shader_module(self.vert, None);
    device.destroy_shader_module(self.frag, None);
  }
}

impl Shader {
  pub fn get_pipeline_shader_creation_info(&self) -> [vk::PipelineShaderStageCreateInfo; 2] {
    [
      vk::PipelineShaderStageCreateInfo {
        // Vertex shader
        s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
        p_next: ptr::null(),
        flags: vk::PipelineShaderStageCreateFlags::empty(),
        module: self.vert,
        p_name: MAIN_FN_NAME.as_ptr(),
        p_specialization_info: ptr::null(),
        stage: vk::ShaderStageFlags::VERTEX,
      },
      vk::PipelineShaderStageCreateInfo {
        // Fragment shader
        s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
        p_next: ptr::null(),
        flags: vk::PipelineShaderStageCreateFlags::empty(),
        module: self.frag,
        p_name: MAIN_FN_NAME.as_ptr(),
        p_specialization_info: ptr::null(),
        stage: vk::ShaderStageFlags::FRAGMENT,
      },
    ]
  }
}
