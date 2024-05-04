use ash::vk;

use crate::render::{
  initialization::InstanceCreationError,
  pipelines::{PipelineCacheError, PipelineCreationError},
};

use super::swapchain::SwapchainCreationError;

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

#[derive(thiserror::Error, Debug)]
pub enum OutOfMemoryError {
  #[error("Out of Device Memory")]
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
  InstanceCreationFailed(#[source] InstanceCreationError),

  #[error("No physical device supports the application")]
  NoCompatibleDevices,

  #[error("Not enough memory")]
  NotEnoughMemory(#[source] Option<AllocationError>),

  #[error("Failed to create swapchain")]
  SwapchainCreationFailed(#[source] SwapchainCreationError),

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

impl From<InstanceCreationError> for InitializationError {
  fn from(value: InstanceCreationError) -> Self {
    InitializationError::InstanceCreationFailed(value)
  }
}

impl From<PipelineCreationError> for InitializationError {
  fn from(value: PipelineCreationError) -> Self {
    InitializationError::PipelineCreationFailed(value)
  }
}

impl From<SwapchainCreationError> for InitializationError {
  fn from(value: SwapchainCreationError) -> Self {
    InitializationError::SwapchainCreationFailed(value)
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

impl From<AllocationError> for InitializationError {
  fn from(value: AllocationError) -> Self {
    match value {
      AllocationError::NotEnoughMemory(_) => InitializationError::NotEnoughMemory(Some(value)),
      AllocationError::DeviceIsLost => InitializationError::DeviceLost,
      AllocationError::MemoryMapFailed => InitializationError::MemoryMapFailed,
      _ => {
        log::error!(
          "Allocation error failed because of an unhandled case: {:?}",
          value
        );
        InitializationError::NotEnoughMemory(Some(value))
      }
    }
  }
}

impl From<OutOfMemoryError> for InitializationError {
  fn from(_value: OutOfMemoryError) -> Self {
    InitializationError::NotEnoughMemory(None)
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

#[derive(thiserror::Error, Debug)]
pub enum AllocationError {
  #[error("No memory type supports all buffers and images")]
  NoMemoryTypeSupportsAll,
  #[error("Allocation size ({0}) exceeds value allowed by the device")]
  TotalSizeExceedsAllowed(u64),
  // allocation size is bigger than each supported heap size
  #[error("Allocation size ({0}) is bigger than the capacity of each supported heap")]
  TooBigForAllSupportedHeaps(u64),
  #[error("Not enough memory")]
  NotEnoughMemory(#[source] OutOfMemoryError),
  #[error("Failed to map necessary memory")]
  MemoryMapFailed,
  #[error("Device is lost")]
  DeviceIsLost,
}

impl From<vk::Result> for AllocationError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        AllocationError::NotEnoughMemory(OutOfMemoryError::from(value))
      }
      vk::Result::ERROR_MEMORY_MAP_FAILED => AllocationError::MemoryMapFailed,
      vk::Result::ERROR_DEVICE_LOST => AllocationError::DeviceIsLost,
      _ => panic!("Invalid cast from vk::Result to AllocationError"),
    }
  }
}

impl From<OutOfMemoryError> for AllocationError {
  fn from(value: OutOfMemoryError) -> Self {
    AllocationError::NotEnoughMemory(value)
  }
}
