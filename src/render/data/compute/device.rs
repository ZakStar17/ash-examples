use std::{mem::offset_of, ops::BitOr, ptr::NonNull};

use ash::vk;
use rand::rngs::ThreadRng;

use crate::{
  render::{
    allocator::allocate_and_bind_memory,
    create_objs::create_buffer,
    device_destroyable::{create_destroyable_array, destroy, DeviceManuallyDestroyed},
    errors::AllocationError,
    initialization::device::{Device, PhysicalDevice},
    FRAMES_IN_FLIGHT,
  },
  utility::{self, OnErr},
};

#[derive(Debug)]
pub struct DeviceComputeData {
  pub memory: vk::DeviceMemory,
  pub instance_capacity: u64,
  // instance data that always belongs to the compute family
  pub instance_compute: [vk::Buffer; FRAMES_IN_FLIGHT],
  // instance data that gets copied from instance_compute to be used in graphics
  pub instance_graphics: [vk::Buffer; FRAMES_IN_FLIGHT],
  // cpus generate (pseudo)random values more easily than gpus, so these are generated in
  // cpu memory and then copied to the gpu
  pub device_random_values: [vk::Buffer; FRAMES_IN_FLIGHT],
}
