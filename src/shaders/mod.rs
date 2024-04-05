use std::{fs::File, io::Read, path::Path, ptr};

use ash::vk;

pub mod shader;

pub use shader::Shader;

use crate::{errors::OutOfMemoryError, utility::error_chain_fmt};

#[derive(thiserror::Error)]
pub enum ShaderError {
  #[error("IOError")]
  IO(#[source] std::io::Error),
  #[error("Out of memory")]
  OutOfMemory(#[source] OutOfMemoryError),
  #[error("Failed to compile")]
  InvalidShader
}
impl std::fmt::Debug for ShaderError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}
impl From<vk::Result> for ShaderError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY => ShaderError::OutOfMemory(value.into()),
      vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => ShaderError::OutOfMemory(value.into()),
      vk::Result::ERROR_INVALID_SHADER_NV => ShaderError::InvalidShader,
      _ => panic!("Attempted invalid vk::Result cast")
    }
  }
}
impl From<std::io::Error> for ShaderError {
  fn from(value: std::io::Error) -> Self {
    Self::IO(value)
  }
}

pub fn load_shader(device: &ash::Device, shader_path: &Path) -> Result<vk::ShaderModule, ShaderError> {
  log::info!("Loading {:?}", shader_path);
  let code = read_shader_code(shader_path)?;
  create_shader_module(device, code).map_err(|err| err.into())
}

fn read_shader_code(shader_path: &Path) -> std::io::Result<Vec<u8>> {
  let mut file =
    File::open(shader_path)?;

  let mut bytes = Vec::new();
  file
    .read_to_end(&mut bytes)?;

  Ok(bytes)
}

fn create_shader_module(device: &ash::Device, code: Vec<u8>) -> Result<vk::ShaderModule, vk::Result> {
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
  }
}
