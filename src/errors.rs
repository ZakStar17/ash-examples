use ash::vk;

use crate::{
  instance::InstanceCreationError, pipeline_cache::PipelineCacheError, utility::error_chain_fmt,
};

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

#[derive(thiserror::Error, Debug)]
pub enum InitializationError {
  #[error("Instance creation failed")]
  InstanceCreationFailed(#[source] InstanceCreationError),

  #[error("No physical device supports the application")]
  NoCompatibleDevices,

  #[error("Not enough memory")]
  NotEnoughMemory(#[source] Option<AllocationError>),

  #[error("IO error")]
  IOError(#[source] std::io::Error),

  // undefined behavior / driver or application bug (see vl)
  #[error("Device is lost")]
  DeviceLost,
  #[error("Unknown")]
  Unknown,
}

impl From<InstanceCreationError> for InitializationError {
  fn from(value: InstanceCreationError) -> Self {
    InitializationError::InstanceCreationFailed(value)
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
      _ => {
        log::error!("Invalid vk::Result: {:?}", value);
        InitializationError::Unknown
      }
    }
  }
}

impl From<OutOfMemoryError> for InitializationError {
  fn from(_value: OutOfMemoryError) -> Self {
    InitializationError::NotEnoughMemory(None)
  }
}

impl From<AllocationError> for InitializationError {
  fn from(value: AllocationError) -> Self {
    match value {
      AllocationError::NotEnoughMemory(_) => {}
      _ => {
        log::error!(
          "Allocation error failed because of an unhandled case: {:?}",
          value
        );
      }
    }
    InitializationError::NotEnoughMemory(Some(value))
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
}

impl From<vk::Result> for AllocationError {
  fn from(value: vk::Result) -> Self {
    AllocationError::NotEnoughMemory(OutOfMemoryError::from(value))
  }
}

impl From<OutOfMemoryError> for AllocationError {
  fn from(value: OutOfMemoryError) -> Self {
    AllocationError::NotEnoughMemory(value)
  }
}
