use std::path::Path;

use ash::vk;

const SHADER_PATH: &'static str = "./compiled_shaders/shader.spv";

// same as declared in the shader
pub const SHADER_GROUP_SIZE_X: u32 = 16;
pub const SHADER_GROUP_SIZE_Y: u32 = 16;

pub struct Shader {
  pub module: vk::ShaderModule,
}

impl Shader {
  pub fn load(device: &ash::Device) -> Self {
    Self {
      module: super::load_shader(device, Path::new(SHADER_PATH)),
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_shader_module(self.module, None);
  }
}
