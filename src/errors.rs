use ash::vk;

use crate::{
  allocator::{AllocationError, DeviceMemoryInitializationError},
  initialization::{
    device::{DeviceCreationError, DeviceSelectionError},
    InstanceCreationError,
  },
  pipelines::{PipelineCacheError, PipelineCreationError},
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

#[derive(thiserror::Error, Debug, Clone, Copy)]
#[error("Vulkan returned ERROR_DEVICE_LOST. See https://docs.vulkan.org/spec/latest/chapters/devsandqueues.html#devsandqueues-lost-device")]
pub struct DeviceIsLost;

#[derive(thiserror::Error, Debug, Clone, Copy)]
pub enum QueueSubmitError {
  #[error(transparent)]
  OutOfMemory(#[from] OutOfMemoryError),
  #[error(transparent)]
  DeviceIsLost(#[from] DeviceIsLost),
}

impl From<vk::Result> for QueueSubmitError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_DEVICE_MEMORY | vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
        QueueSubmitError::OutOfMemory(value.into())
      }
      vk::Result::ERROR_DEVICE_LOST => QueueSubmitError::DeviceIsLost(DeviceIsLost {}),
      _ => {
        panic!("Invalid vk::Result to QueueSubmitError cast: {:?}", value);
      }
    }
  }
}

impl From<QueueSubmitError> for DeviceMemoryInitializationError {
  fn from(value: QueueSubmitError) -> Self {
    match value {
      QueueSubmitError::DeviceIsLost(_) => {
        DeviceMemoryInitializationError::DeviceIsLost(DeviceIsLost {})
      }
      QueueSubmitError::OutOfMemory(v) => v.into(),
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
  AllocationError(#[from] DeviceMemoryInitializationError),
  #[error("Failed to submit some queue: {0}")]
  QueueSubmissionError(#[from] QueueSubmitError),
  #[error("Failed to create pipelines:\n{0}")]
  PipelineCreationFailed(#[from] PipelineCreationError),
  #[error("An error occurred when creating or saving the pipeline cache: {0}")]
  PipelineCacheError(#[from] PipelineCacheError),

  #[error(transparent)]
  IOError(#[from] std::io::Error),
}

impl From<AllocationError> for InitializationError {
  fn from(value: AllocationError) -> Self {
    Self::AllocationError(value.into())
  }
}
