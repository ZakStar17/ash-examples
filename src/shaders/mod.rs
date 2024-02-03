use std::{fs::File, io::Read, path::Path, ptr};

use ash::vk;

pub mod shader;

pub use shader::Shader;

pub fn load_shader(device: &ash::Device, shader_path: &Path) -> vk::ShaderModule {
  let code = read_shader_code(shader_path);
  create_shader_module(device, code)
}

fn read_shader_code(shader_path: &Path) -> Vec<u8> {
  let mut file =
    File::open(shader_path).expect(&format!("Failed to find spv file at {:?}", shader_path));

  let mut bytes = Vec::new();
  file
    .read_to_end(&mut bytes)
    .expect("Failed to read shader file");
  bytes
}

fn create_shader_module(device: &ash::Device, code: Vec<u8>) -> vk::ShaderModule {
  let shader_module_create_info = vk::ShaderModuleCreateInfo {
    s_type: vk::StructureType::SHADER_MODULE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::ShaderModuleCreateFlags::empty(),
    code_size: code.len(),
    p_code: code.as_ptr() as *const u32,
  };

  unsafe {
    device
      .create_shader_module(&shader_module_create_info, None)
      .expect("Failed to create shader module")
  }
}
