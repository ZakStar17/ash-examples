use ash::vk;

use crate::{
  allocator::MemoryInitializationError,
  initialization::InstanceCreationError,
  pipelines::{PipelineCacheError, PipelineCreationError},
};

pub fn error_chain_fmt(
  e: &impl std::error::Error,
  f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
  writeln!(f, "{}\nCauses:", e)?;
  let mut current = e.source();
  while let Some(cause) = current {
    writeln!(f, "  {}", cause)?;
    current = cause.source();
  }
  Ok(())
}

#[derive(thiserror::Error, Debug, Clone, Copy)]
pub enum OutOfMemoryError {
  #[error("Out of device memory")]
  OutOfDeviceMemory,
  #[error("Out of host memory")]
  OutOfHostMemory,
}

impl From<vk::Result> for OutOfMemoryError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemoryError::OutOfDeviceMemory,
      vk::Result::ERROR_OUT_OF_HOST_MEMORY => OutOfMemoryError::OutOfHostMemory,
      _ => {
        panic!("Invalid vk::Result to OutOfMemoryError cast: {:?}", value);
      }
    }
  }
}

impl From<OutOfMemoryError> for vk::Result {
  fn from(value: OutOfMemoryError) -> Self {
    match value {
      OutOfMemoryError::OutOfDeviceMemory => vk::Result::ERROR_OUT_OF_DEVICE_MEMORY,
      OutOfMemoryError::OutOfHostMemory => vk::Result::ERROR_OUT_OF_HOST_MEMORY,
    }
  }
}

#[derive(thiserror::Error)]
pub enum InitializationError {
  #[error("Instance creation failed: {0}")]
  InstanceCreationFailed(#[from] InstanceCreationError),

  #[error("No physical device is compatible with the application requirements")]
  NoCompatibleDevices,

  #[error("Failed to allocate memory for some buffer or image\n{0}")]
  AllocationFailed(#[from] MemoryInitializationError),

  #[error("Ran out of memory while issuing some command or creating memory: {0}")]
  GenericOutOfMemoryError(#[from] OutOfMemoryError),

  #[error("Failed to create pipelines:\n{0}")]
  PipelineCreationFailed(#[from] PipelineCreationError),

  #[error(transparent)]
  IOError(#[from] std::io::Error),

  // undefined behavior / driver or application bug (see vl)
  #[error("Vulkan returned ERROR_DEVICE_LOST")]
  DeviceLost,
  #[error("Vulkan returned ERROR_UNKNOWN")]
  Unknown,
}
impl std::fmt::Debug for InitializationError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}

impl From<PipelineCacheError> for InitializationError {
  fn from(value: PipelineCacheError) -> Self {
    match value {
      PipelineCacheError::IOError(err) => InitializationError::IOError(err),
      PipelineCacheError::OutOfMemoryError(err) => InitializationError::from(err),
    }
  }
}

impl From<vk::Result> for InitializationError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_DEVICE_MEMORY | vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
        OutOfMemoryError::from(value).into()
      }
      vk::Result::ERROR_DEVICE_LOST => InitializationError::DeviceLost,
      vk::Result::ERROR_UNKNOWN => InitializationError::Unknown,
      // validation layers may say more on this
      vk::Result::ERROR_INITIALIZATION_FAILED => InitializationError::Unknown,
      _ => {
        log::error!(
          "Unhandled vk::Result {} during general initialization",
          value
        );
        InitializationError::Unknown
      }
    }
  }
}
