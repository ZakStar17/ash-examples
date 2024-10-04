use ash::vk;

use crate::{
  allocator::AllocationError,
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
  #[error("Instance creation failed")]
  InstanceCreationFailed(#[from] InstanceCreationError),

  #[error("No physical device supports the application")]
  NoCompatibleDevices,

  #[error("Not enough memory / memory allocation failed")]
  NotEnoughMemory(#[source] Option<AllocationError>),

  #[error("Failed to create pipelines")]
  PipelineCreationFailed(#[source] PipelineCreationError),

  #[error("Memory map failed: The application was unable to map the require memory")]
  MemoryMapFailed,

  #[error("IO error")]
  IOError(#[source] std::io::Error),

  // undefined behavior / driver or application bug (see vl)
  #[error("Device is lost")]
  DeviceLost,
  #[error("Unknown")]
  Unknown,
}
impl std::fmt::Debug for InitializationError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}

impl From<AllocationError> for InitializationError {
  fn from(value: AllocationError) -> Self {
    Self::NotEnoughMemory(Some(value))
  }
}

impl From<PipelineCreationError> for InitializationError {
  fn from(value: PipelineCreationError) -> Self {
    InitializationError::PipelineCreationFailed(value)
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

impl From<OutOfMemoryError> for InitializationError {
  fn from(_value: OutOfMemoryError) -> Self {
    InitializationError::NotEnoughMemory(None)
  }
}

impl From<vk::Result> for InitializationError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_DEVICE_MEMORY | vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
        InitializationError::NotEnoughMemory(None)
      }
      vk::Result::ERROR_DEVICE_LOST => InitializationError::DeviceLost,
      vk::Result::ERROR_UNKNOWN => InitializationError::Unknown,
      // validation layers may say more on this
      vk::Result::ERROR_INITIALIZATION_FAILED => InitializationError::Unknown,
      vk::Result::ERROR_MEMORY_MAP_FAILED => InitializationError::MemoryMapFailed,
      _ => {
        log::error!("Invalid vk::Result: {:?}", value);
        InitializationError::Unknown
      }
    }
  }
}
