use std::path::Path;

use ash::vk;

const SHADER_PATH: &'static str = "./shaders/shader.spv";

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
