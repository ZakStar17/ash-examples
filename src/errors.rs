use ash::vk;

use crate::utility::error_chain_fmt;

#[derive(thiserror::Error)]
pub enum InitializationError {
  #[error("No physical device supports the application")]
  NoCompatibleDevices,

  // can by the most part happen anytime
  #[error("Out of device memory")]
  NotEnoughDeviceMemory(#[source] Option<AllocationError>),
  #[error("Out of host memory")]
  NotEnoughHostMemory(#[source] Option<AllocationError>),

  // undefined behavior / driver bug (see vl)
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

impl From<vk::Result> for InitializationError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => InitializationError::NotEnoughDeviceMemory(None),
      vk::Result::ERROR_OUT_OF_HOST_MEMORY => InitializationError::NotEnoughHostMemory(None),
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

impl From<AllocationError> for InitializationError {
  fn from(value: AllocationError) -> Self {
    match value {
      AllocationError::NotEnoughDeviceMemory => {
        InitializationError::NotEnoughDeviceMemory(Some(value))
      }
      AllocationError::NotEnoughHostMemory => InitializationError::NotEnoughHostMemory(Some(value)),
      _ => {
        log::error!(
          "Allocation error failed because of an unhandled case: {:?}",
          value
        );
        InitializationError::Unknown
      }
    }
  }
}

#[derive(thiserror::Error)]
pub enum AllocationError {
  #[error("No memory type supports all buffers and images")]
  NoMemoryTypeSupportsAll,
  #[error("Allocation size ({0}) exceeds value allowed by the device")]
  TotalSizeExceedsAllowed(u64),
  // allocation size is bigger than each supported heap size
  #[error("Allocation size ({0}) is bigger than the capacity of each supported heap")]
  TooBigForAllSupportedHeaps(u64),
  #[error("Not enough device memory")]
  NotEnoughDeviceMemory,
  #[error("Not enough host memory")]
  NotEnoughHostMemory,
}
impl std::fmt::Debug for AllocationError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}

impl From<vk::Result> for AllocationError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => AllocationError::NotEnoughDeviceMemory,
      vk::Result::ERROR_OUT_OF_HOST_MEMORY => AllocationError::NotEnoughHostMemory,
      _ => panic!(),
    }
  }
}
