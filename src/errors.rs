use ash::vk;

use crate::{allocator::AllocationError, instance::InstanceCreationError};

#[derive(thiserror::Error, Debug, Clone, Copy)]
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
  InstanceCreationFailed(#[from] InstanceCreationError),

  #[error("No physical device supports the application")]
  NoCompatibleDevices,

  #[error("Not enough memory / memory allocation failed")]
  NotEnoughMemory(#[source] Option<AllocationError>),

  // undefined behavior / driver or application bug (see vl)
  #[error("Device is lost")]
  DeviceLost,
  #[error("Unknown")]
  Unknown,
}

impl From<AllocationError> for InitializationError {
  fn from(value: AllocationError) -> Self {
    Self::NotEnoughMemory(Some(value))
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
