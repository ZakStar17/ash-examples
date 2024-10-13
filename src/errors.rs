use ash::vk;

use crate::{
  allocator::AllocationError,
  device::{DeviceCreationError, DeviceSelectionError},
  instance::InstanceCreationError,
};

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

#[derive(thiserror::Error, Debug)]
pub enum InitializationError {
  #[error("Instance creation failed:\n    {0}")]
  InstanceCreationFailed(#[from] InstanceCreationError),

  #[error("An error occurred during device selection: {0}")]
  DeviceSelectionError(#[from] DeviceSelectionError),
  #[error("No physical device supports the application")]
  NoCompatibleDevices,
  #[error("An error occurred during the creation of the logical device:\n    {0}")]
  DeviceCreationError(#[from] DeviceCreationError),

  #[error("Some command failed because of a generic OutOfMemory error: {0}")]
  OutOfMemoryError(#[from] OutOfMemoryError),
  #[error("Failed to allocate device memory:\n    ")]
  AllocationError(#[from] AllocationError),
}
