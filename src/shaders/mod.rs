use std::{fs::File, io, io::Read, path::Path};

use ash::vk;

pub mod shader;

pub use shader::Shader;

use crate::errors::OutOfMemoryError;

#[derive(thiserror::Error, Debug)]
pub enum ShaderError {
  #[error("IO error")]
  IOError(#[source] io::Error),

  #[error("Failed to compile or link")]
  Invalid,

  #[error("Not enough memory")]
  NotEnoughMemory(#[source] OutOfMemoryError),
}

impl From<io::Error> for ShaderError {
  fn from(value: io::Error) -> Self {
    ShaderError::IOError(value)
  }
}

pub fn load_shader(
  device: &ash::Device,
  shader_path: &Path,
) -> Result<vk::ShaderModule, ShaderError> {
  let code = read_shader_code(shader_path)?;
  create_shader_module(device, &code)
}

fn read_shader_code(shader_path: &Path) -> io::Result<Vec<u32>> {
  let mut file = File::open(shader_path)?;

  let mut bytes = Vec::new();
  file.read_to_end(&mut bytes)?;

  bytes.shrink_to_fit();
  assert!(bytes.capacity() % 4 == 0);

  let (ptr, len, capacity) = bytes.into_raw_parts();
  Ok(unsafe { Vec::from_raw_parts(ptr as *mut u32, len / 4, capacity / 4) })
}

fn create_shader_module(
  device: &ash::Device,
  code: &[u32],
) -> Result<vk::ShaderModule, ShaderError> {
  let create_info = vk::ShaderModuleCreateInfo::default().code(code);

  unsafe { device.create_shader_module(&create_info, None) }.map_err(|vkerr| match vkerr {
    vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
      ShaderError::NotEnoughMemory(vkerr.into())
    }
    vk::Result::ERROR_INVALID_SHADER_NV => ShaderError::Invalid,
    _ => panic!(),
  })
}
