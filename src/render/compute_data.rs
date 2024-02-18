use std::{
  mem::{offset_of, size_of},
  ops::BitOr,
  ptr::NonNull,
};

use ash::vk;
use rand::{rngs::ThreadRng, Rng};

use crate::utility;

use super::{
  objects::{
    allocations::{allocate_and_bind_memory, create_buffer},
    device::PhysicalDevice,
  },
  FRAMES_IN_FLIGHT,
};

pub const PUSH_CONSTANT_NEXT_PROJECTILES_COUNT: usize = 4;

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
  pub delta_time: f32,

  pub target_projectile_count: u32,
  pub cur_projectile_count: u32, // size: 5

  _padding0: [u32; 3], // size: 8
  pub next_projectiles: [Projectile; PUSH_CONSTANT_NEXT_PROJECTILES_COUNT],
}

// host accessible data after shader dispatch
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ComputeOutput {
  colliding: u32,
  pc_next_projectiles_i: u32,
  new_projectile_count: u32,
}

pub struct ComputeData {
  random: ThreadRng,

  pub host_memory: vk::DeviceMemory,
  pub shader_output: [vk::Buffer; FRAMES_IN_FLIGHT],
  pub shader_output_data: [NonNull<ComputeOutput>; FRAMES_IN_FLIGHT],

  pub device_memory: vk::DeviceMemory,
  pub instance_compute: [vk::Buffer; FRAMES_IN_FLIGHT],
  pub instance_graphics: [vk::Buffer; FRAMES_IN_FLIGHT],

  pub instance_size: u64,

  pub constants: ComputePushConstants,
}

impl ComputeData {
  pub const COMPUTE_BUFFER_COUNT: u32 = 4;

  pub fn new(device: &ash::Device, physical_device: &PhysicalDevice) -> Self {
    let shader_output = utility::populate_array_with_expression!(
      create_buffer(
        device,
        size_of::<ComputeOutput>() as u64,
        // transfer dst is used in a buffer clear command
        vk::BufferUsageFlags::STORAGE_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      ),
      FRAMES_IN_FLIGHT
    );
    log::debug!("Allocating memory for compute output buffers");
    let host_alloc = allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE,
      vk::MemoryPropertyFlags::HOST_CACHED,
      &shader_output,
      &[],
    )
    .unwrap();

    let host_ptr = unsafe {
      device
        .map_memory(
          host_alloc.memory,
          0,
          vk::WHOLE_SIZE,
          vk::MemoryMapFlags::empty(),
        )
        .expect("...") as *mut u8
    };
    let offsets = host_alloc.offsets.buffer_offsets();
    let mut shader_output_data = unsafe {
      [
        NonNull::new_unchecked(host_ptr.add(offsets[0] as usize) as *mut ComputeOutput),
        NonNull::new_unchecked(host_ptr.add(offsets[1] as usize) as *mut ComputeOutput),
      ]
    };

    // clear outputs for first shader update
    unsafe {
      *shader_output_data[0].as_mut() = ComputeOutput::default();
      *shader_output_data[1].as_mut() = ComputeOutput::default();
    }

    let instance_size = (size_of::<Projectile>() * 4000) as u64;
    let instance_compute = utility::populate_array_with_expression!(
      create_buffer(
        device,
        instance_size,
        vk::BufferUsageFlags::STORAGE_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_SRC),
      ),
      FRAMES_IN_FLIGHT
    );
    let instance_graphics = utility::populate_array_with_expression!(
      create_buffer(
        device,
        instance_size,
        vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      ),
      FRAMES_IN_FLIGHT
    );
    log::debug!("Allocating memory for instance buffers");
    let device_alloc = allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      vk::MemoryPropertyFlags::empty(),
      &[instance_compute[0], instance_compute[1], instance_graphics[0], instance_graphics[1]],
      &[],
    )
    .unwrap();

    let mut constants = ComputePushConstants::default();
    constants.target_projectile_count = 2000;
    let mut random = rand::thread_rng();
    for i in 0..PUSH_CONSTANT_NEXT_PROJECTILES_COUNT {
      let new_proj = Projectile {
        pos: [(random.gen::<f32>() - 0.5) * 2.0, -1.0],
        vel: [0.0, -1.0],
      };
      constants.next_projectiles[i] = new_proj;
    }

    Self {
      random,

      host_memory: host_alloc.memory,
      shader_output,
      shader_output_data,

      device_memory: device_alloc.memory,
      instance_compute,
      instance_graphics,

      instance_size,

      constants,
    }
  }

  // maximum projectile count that can be valid after a shader invocation
  pub fn max_valid_projectile_count(&self) -> usize {
    (self.constants.target_projectile_count as usize)
      .min(self.constants.cur_projectile_count as usize + PUSH_CONSTANT_NEXT_PROJECTILES_COUNT)
  }

  pub fn update(&mut self, frame_i: usize, delta_time: f32, player_position: [f32; 2]) {
    self.constants.delta_time = delta_time;
    self.constants.player_pos = player_position;

    let output: ComputeOutput = unsafe { self.shader_output_data[frame_i].as_ref().clone() };
    println!("output {:#?}", output);

    debug_assert!(output.new_projectile_count as usize <= PUSH_CONSTANT_NEXT_PROJECTILES_COUNT);

    self.constants.cur_projectile_count += output.new_projectile_count;

    let next_proj_i = output.pc_next_projectiles_i;
    for i in 0..((next_proj_i as usize).min(PUSH_CONSTANT_NEXT_PROJECTILES_COUNT)) {
      let new_proj = Projectile {
        pos: [(self.random.gen::<f32>() - 0.5) * 2.0, -1.0],
        vel: [0.0, -1.0],
      };
      self.constants.next_projectiles[i] = new_proj;
    }

    println!("constants {:#?}", self.constants);

    if output.colliding > 0 {
      // temp
      println!("Colliding! {}", output.colliding);
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
