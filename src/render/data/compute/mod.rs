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

// explanations in src/render/shaders/compute/shader.comp
#[repr(C)]
#[derive(Debug, Default)]
pub struct ComputePushConstants {
  pub player_pos: [f32; 2], // size: 2
  pub delta_time: f32,      // size: 3

  pub bullet_count: u32, // size: 4
  pub target_bullet_count: u32,
  pub random_uniform_reserved_index: u32, // size: 6
}

// equal to src/render/shaders/compute/shader.comp
// see shader code for more information about fields
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ComputeHostIO {
  // 1 if colliding with bullet, 0 otherwise
  pub colliding: u32,
  pub random_uniform_index: u32,
}

// WARNING: should equal to what in the shader
// basically the total number of new_projectiles divided bt RANDOM_VALUES_PER_BULLET
// that can be added each frame
const STAGING_RANDOM_VALUES_COUNT: usize = 16384;

#[derive(Debug)]
pub struct ComputeData {
  pub host: HostComputeData,
  pub device: DeviceComputeData,

  pub bullet_count: usize,
  pub target_bullet_count: usize,
}

impl ComputeData {
  pub fn new(device: &Device, physical_device: &PhysicalDevice) -> Result<Self, AllocationError> {
    let host = HostComputeData::create_and_allocate(device, physical_device)?;
    let device = DeviceComputeData::create_and_allocate(device, physical_device)?;

    Ok(Self {
      host,
      device,
      bullet_count: 0,
      target_bullet_count: 12,
    })
  }
}

impl DeviceManuallyDestroyed for ComputeData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.host.destroy_self(device);
    self.device.destroy_self(device);
  }
}
