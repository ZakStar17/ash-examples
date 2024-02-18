use std::{
  mem::{offset_of, size_of},
  ops::BitOr,
};

use ash::vk;

use crate::utility;

use super::{
  objects::{
    allocations::{allocate_and_bind_memory, create_buffer},
    device::PhysicalDevice,
  },
  FRAMES_IN_FLIGHT,
};

const PUSH_CONSTANT_NEXT_PROJECTILES_COUNT: usize = 4;

// all data passed to the shader follows std430 layout rules
// https://www.oreilly.com/library/view/opengl-programming-guide/9780132748445/app09lev1sec3.html

// size and alignment: 4
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Projectile {
  pos: [f32; 2],
  vel: [f32; 2],
}

// impl instance vertex for Projectile
impl Projectile {
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
        location: offset,
        binding,
        format: vk::Format::R32G32_SFLOAT,
        offset: offset_of!(Self, vel) as u32,
      },
    ]
  }
}

#[repr(C)]
// structure and layout equal to what is defined in the shader
pub struct ComputePushConstants {
  player_pos: [f32; 2], // size: 2
  delta_time: f32,

  target_projectile_count: u32,
  cur_projectile_count: u32, // size: 5

  _padding0: [u32; 3], // size: 8
  next_projectiles: [Projectile; PUSH_CONSTANT_NEXT_PROJECTILES_COUNT],
}

// host accessible data after shader dispatch
#[repr(C)]
pub struct ComputeOutput {
  colliding: u32,
  pc_next_projectiles_i: u32,
  new_projectile_count: u32,
}

impl ComputeOutput {
  pub fn init() -> Self {
    Self {
      colliding: 0,
      pc_next_projectiles_i: 0,
      new_projectile_count: 0,
    }
  }
}

pub struct ComputeData {
  pub host_memory: vk::DeviceMemory,
  pub shader_output: [vk::Buffer; FRAMES_IN_FLIGHT],

  pub device_memory: vk::DeviceMemory,
  pub instance_compute: [vk::Buffer; FRAMES_IN_FLIGHT],
  pub instance_graphics: [vk::Buffer; FRAMES_IN_FLIGHT],
}

impl ComputeData {
  pub fn new(device: &ash::Device, physical_device: &PhysicalDevice) -> Self {
    let shader_output = utility::populate_array_with_expression!(
      create_buffer(
        device,
        size_of::<ComputeOutput>() as u64,
        vk::BufferUsageFlags::STORAGE_BUFFER,
      ),
      FRAMES_IN_FLIGHT
    );
    let host_alloc = allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE,
      vk::MemoryPropertyFlags::HOST_CACHED,
      &shader_output,
      &[],
    )
    .unwrap();

    let instance_compute = utility::populate_array_with_expression!(
      create_buffer(
        device,
        (size_of::<Projectile>() * 4000) as u64,
        vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_SRC),
      ),
      FRAMES_IN_FLIGHT
    );
    let instance_graphics = utility::populate_array_with_expression!(
      create_buffer(
        device,
        (size_of::<Projectile>() * 4000) as u64,
        vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      ),
      FRAMES_IN_FLIGHT
    );
    let device_buffers = utility::copy_iter_into_array!(
      instance_compute
        .clone()
        .into_iter()
        .chain(instance_graphics.clone().into_iter()),
      FRAMES_IN_FLIGHT * 2
    );
    let device_alloc = allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      vk::MemoryPropertyFlags::empty(),
      &device_buffers,
      &[],
    ).unwrap();

    Self {
      host_memory: host_alloc.memory,
      shader_output,

      device_memory: device_alloc.memory,
      instance_compute,
      instance_graphics,
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    for i in 0..FRAMES_IN_FLIGHT {
      device.destroy_buffer(self.shader_output[i], None);
      device.destroy_buffer(self.instance_compute[i], None);
      device.destroy_buffer(self.instance_graphics[i], None);
    }
    device.free_memory(self.host_memory, None);
    device.free_memory(self.device_memory, None);
  }
}
