mod device;
mod host;

use std::mem::offset_of;

use ash::vk;
use device::DeviceComputeData;
use host::HostComputeData;
use rand::rngs::ThreadRng;

use crate::render::{
  device_destroyable::DeviceManuallyDestroyed,
  errors::AllocationError,
  initialization::device::{Device, PhysicalDevice},
};

pub const MAX_NEW_BULLETS_PER_FRAME: usize = 1024;
pub const INITIAL_MAX_BULLET_COUNT: usize = 1024;

// all data passed to the shader follows std430 layout rules
// https://www.oreilly.com/library/view/opengl-programming-guide/9780132748445/app09lev1sec3.html

// size and alignment: 4
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Bullet {
  pos: [f32; 2],
  vel: [f32; 2],
}

// impl instance vertex for Bullet
impl Bullet {
  const ATTRIBUTE_SIZE: usize = 2;

  pub const fn get_binding_description(binding: u32) -> vk::VertexInputBindingDescription {
    vk::VertexInputBindingDescription {
      binding,
      stride: size_of::<Self>() as u32,
      input_rate: vk::VertexInputRate::INSTANCE,
    }
  }

  pub const fn get_attribute_descriptions(
    offset: u32,
    binding: u32,
  ) -> [vk::VertexInputAttributeDescription; Self::ATTRIBUTE_SIZE] {
    [
      vk::VertexInputAttributeDescription {
        location: offset,
        binding,
        format: vk::Format::R32G32_SFLOAT,
        offset: offset_of!(Self, pos) as u32,
      },
      vk::VertexInputAttributeDescription {
        location: offset + 1,
        binding,
        format: vk::Format::R32G32_SFLOAT,
        offset: offset_of!(Self, vel) as u32,
      },
    ]
  }
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct ComputePushConstants {
  pub player_pos: [f32; 2], // size: 2
  pub delta_time: f32,      // size: 1

  pub bullet_count: u32, // size: 4
}

// host accessible data after shader dispatch
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ComputeOutput {
  // 1 if colliding with bullet, 0 otherwise
  colliding: u32,
  // number of random values remaining for use in ComputeData::device_random_values
  random_values_left: u32,
}

#[derive(Debug)]
pub struct ComputeData {
  host: HostComputeData,
  device: DeviceComputeData,

  rng: ThreadRng,
}

impl ComputeData {
  pub fn new(device: &Device, physical_device: &PhysicalDevice) -> Result<Self, AllocationError> {
    let host = HostComputeData::create_and_allocate(device, physical_device)?;
    let device = DeviceComputeData::create_and_allocate(device, physical_device)?;

    let rng = rand::thread_rng();

    Ok(Self { host, device, rng })
  }
}

impl DeviceManuallyDestroyed for ComputeData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.host.destroy_self(device);
    self.device.destroy_self(device);
  }
}
