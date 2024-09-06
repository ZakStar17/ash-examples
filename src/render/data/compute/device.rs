use std::ops::BitOr;

use ash::vk;

use crate::{
  render::{
    allocator::allocate_and_bind_memory,
    create_objs::create_buffer,
    data::compute::MAX_RANDOM_VALUES,
    device_destroyable::{
      destroy, fill_destroyable_array_with_expression, DeviceManuallyDestroyed,
    },
    errors::AllocationError,
    initialization::device::{Device, PhysicalDevice},
    FRAMES_IN_FLIGHT,
  },
  utility::{self, OnErr},
};

use super::Bullet;

const INITIAL_INSTANCE_CAPACITY: u64 = 10000;
const INITIAL_INSTANCE_SIZE: u64 = INITIAL_INSTANCE_CAPACITY * size_of::<Bullet>() as u64;

#[derive(Debug)]
pub struct DeviceComputeData {
  pub memory: vk::DeviceMemory,
  // instance data that always belongs to the compute family
  pub instance_compute: [vk::Buffer; FRAMES_IN_FLIGHT],
  // instance data that gets copied from instance_compute to be used in graphics
  pub instance_graphics: [vk::Buffer; FRAMES_IN_FLIGHT],
  // cpus generate (pseudo)random values more easily than gpus, so these are generated in
  // cpu memory and then copied to the gpu
  pub random_values: [vk::Buffer; FRAMES_IN_FLIGHT],
}

impl DeviceComputeData {
  const MEMORY_PRIORITY: f32 = 0.7;

  pub fn create_and_allocate(
    device: &Device,
    physical_device: &PhysicalDevice,
  ) -> Result<Self, AllocationError> {
    let instance_compute = fill_destroyable_array_with_expression!(
      device,
      create_buffer(
        device,
        INITIAL_INSTANCE_SIZE,
        vk::BufferUsageFlags::TRANSFER_SRC.bitor(vk::BufferUsageFlags::STORAGE_BUFFER)
      ),
      FRAMES_IN_FLIGHT
    )?;
    let instance_graphics = fill_destroyable_array_with_expression!(
      device,
      create_buffer(
        device,
        INITIAL_INSTANCE_SIZE,
        vk::BufferUsageFlags::TRANSFER_SRC.bitor(vk::BufferUsageFlags::STORAGE_BUFFER)
      ),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { instance_compute.destroy_self(device) })?;
    let random_values = fill_destroyable_array_with_expression!(
      device,
      create_buffer(
        device,
        MAX_RANDOM_VALUES as u64,
        vk::BufferUsageFlags::TRANSFER_DST.bitor(vk::BufferUsageFlags::UNIFORM_BUFFER)
      ),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe {
      destroy!(device => instance_compute.as_ref(), instance_graphics.as_ref())
    })?;

    let memory = Self::allocate_memory(
      device,
      physical_device,
      instance_compute,
      instance_graphics,
      random_values
    )
    .on_err(|_| unsafe {
      destroy!(device => instance_compute.as_ref(), instance_graphics.as_ref(), random_values.as_ref())
    })?;

    Ok(Self {
      memory,
      instance_compute,
      instance_graphics,
      random_values,
    })
  }

  fn allocate_memory(
    device: &Device,
    physical_device: &PhysicalDevice,
    instance_compute: [vk::Buffer; FRAMES_IN_FLIGHT],
    instance_graphics: [vk::Buffer; FRAMES_IN_FLIGHT],
    random_values: [vk::Buffer; FRAMES_IN_FLIGHT],
  ) -> Result<vk::DeviceMemory, AllocationError> {
    const TOTAL_BUFFERS: usize = FRAMES_IN_FLIGHT * 3;
    let instance_compute_requirements =
      instance_compute.map(|buffer| unsafe { device.get_buffer_memory_requirements(buffer) });
    let instance_graphics_requirements =
      instance_graphics.map(|buffer| unsafe { device.get_buffer_memory_requirements(buffer) });
    let random_values_requirements =
      random_values.map(|buffer| unsafe { device.get_buffer_memory_requirements(buffer) });

    log::debug!("Allocating compute device memory");
    let allocation = allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &utility::concatenate_arrays::<TOTAL_BUFFERS, vk::Buffer>(&[
        &instance_compute,
        &instance_graphics,
        &random_values,
      ]),
      &utility::concatenate_arrays::<TOTAL_BUFFERS, vk::MemoryRequirements>(&[
        &instance_compute_requirements,
        &instance_graphics_requirements,
        &random_values_requirements,
      ]),
      &[],
      &[],
      Self::MEMORY_PRIORITY,
    )?;

    Ok(allocation.memory)
  }
}

impl DeviceManuallyDestroyed for DeviceComputeData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.instance_compute.destroy_self(device);
    self.instance_graphics.destroy_self(device);
    self.random_values.destroy_self(device);

    self.memory.destroy_self(device);
  }
}
