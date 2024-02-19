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

pub const PUSH_CONSTANT_PROJECTILE_REPLACEMENTS_COUNT: usize = 4;
pub const MAX_NEW_PROJECTILES_PER_FRAME: usize = 64;

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

  pub projectile_count: u32, // size: 4
  pub projectile_replacements: [Projectile; PUSH_CONSTANT_PROJECTILE_REPLACEMENTS_COUNT],
}

// host accessible data after shader dispatch
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ComputeOutput {
  colliding: u32,
  // number of projectiles replaced by that shader dispatch
  pc_projectile_replacements_i: u32
}

pub struct OutputBuffer {
  pub buffer: vk::Buffer,
  pub data_ptr: NonNull<ComputeOutput>
}

pub struct NewProjectilesBuffer {
  pub buffer: vk::Buffer,
  pub data_ptr: NonNull<[Projectile; MAX_NEW_PROJECTILES_PER_FRAME]>
}

pub struct ComputeData {
  random: ThreadRng,

  pub host_memory: vk::DeviceMemory,
  pub output: [OutputBuffer; FRAMES_IN_FLIGHT],
  pub new_projectiles: [NewProjectilesBuffer; FRAMES_IN_FLIGHT],

  pub device_memory: vk::DeviceMemory,
  pub instance_capacity: u64,
  pub instance_compute: [vk::Buffer; FRAMES_IN_FLIGHT],
  pub instance_graphics: [vk::Buffer; FRAMES_IN_FLIGHT],

  pub push_data: ComputePushConstants,
  target_projectile_count: usize,
}

impl ComputeData {
  pub const COMPUTE_BUFFER_COUNT: u32 = 4;

  pub fn new(device: &ash::Device, physical_device: &PhysicalDevice) -> Self {
    let new_projectiles_size = size_of::<Projectile>() * MAX_NEW_PROJECTILES_PER_FRAME;
    let shader_output = utility::populate_array_with_expression!(
      create_buffer(
        device,
        size_of::<ComputeOutput>() as u64,
        // transfer dst is used in a buffer clear command
        vk::BufferUsageFlags::STORAGE_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      ),
      FRAMES_IN_FLIGHT
    );
    let new_projectiles = utility::populate_array_with_expression!(
      create_buffer(
        device,
        new_projectiles_size as u64,
        vk::BufferUsageFlags::TRANSFER_SRC,
      ),
      FRAMES_IN_FLIGHT
    );
    log::debug!("Allocating host memory buffers used for the compute shader");
    let host_alloc = allocate_and_bind_memory(
      device,
      physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE,
      vk::MemoryPropertyFlags::HOST_CACHED,
      &[shader_output[0], shader_output[1], new_projectiles[0], new_projectiles[1]],
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
        .unwrap() as *mut u8
    };

    let offsets = host_alloc.offsets.buffer_offsets();
    let mut shader_output_ptrs = unsafe {
      [
        NonNull::new_unchecked(host_ptr.add(offsets[0] as usize) as *mut ComputeOutput),
        NonNull::new_unchecked(host_ptr.add(offsets[1] as usize) as *mut ComputeOutput),
      ]
    };
    let mut new_projectiles_ptrs = unsafe {
      [
        NonNull::new_unchecked(host_ptr.add(offsets[2] as usize) as *mut [Projectile; MAX_NEW_PROJECTILES_PER_FRAME]),
        NonNull::new_unchecked(host_ptr.add(offsets[3] as usize) as *mut [Projectile; MAX_NEW_PROJECTILES_PER_FRAME]),
      ]
    };

    // clear outputs for first shader update
    unsafe {
      *shader_output_ptrs[0].as_mut() = ComputeOutput::default();
      *shader_output_ptrs[1].as_mut() = ComputeOutput::default();
    }

    let initial_capacity = 4000;
    let instance_size = (size_of::<Projectile>() * initial_capacity) as u64;
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

    let mut push_data = ComputePushConstants::default();
    let mut random = rand::thread_rng();
    for i in 0..PUSH_CONSTANT_PROJECTILE_REPLACEMENTS_COUNT {
      push_data.projectile_replacements[i] = Self::random_projectile(&random);
    }

    Self {
      random,

      host_memory: host_alloc.memory,
      output: [OutputBuffer {
        buffer: shader_output[0],
        data_ptr: shader_output_ptrs[0]
      }, OutputBuffer {
        buffer: shader_output[1],
        data_ptr: shader_output_ptrs[1]
      }],
      new_projectiles: [NewProjectilesBuffer {
        buffer: new_projectiles[0],
        data_ptr: new_projectiles_ptrs[0]
      }, NewProjectilesBuffer {
        buffer: new_projectiles[1],
        data_ptr: new_projectiles_ptrs[1]
      }],

      device_memory: device_alloc.memory,
      instance_compute,
      instance_graphics,

      instance_capacity: initial_capacity as u64,
      target_projectile_count: 12,

      push_data,
    }
  }

  fn cur_proj_count(&self) -> usize {
    self.push_data.projectile_count as usize
  }

  fn random_projectile(rng: &ThreadRng) -> Projectile {
    Projectile {
      pos: [(rng.gen::<f32>() - 0.5) * 2.0, -1.0],
      vel: [0.0, 0.1],
    }
  }

  pub fn update(&mut self, frame_i: usize, delta_time: f32, player_position: [f32; 2]) -> ComputeRecordBufferData {
    self.push_data.delta_time = delta_time;
    self.push_data.player_pos = player_position;

    let adding_new_projectiles = self.cur_proj_count() < self.target_projectile_count;
    if self.cur_proj_count() < self.target_projectile_count {
      let cur_new_proj_ref = unsafe {self.new_projectiles[frame_i].data_ptr.as_mut() };
      for i in 0..((self.target_projectile_count - self.cur_proj_count()).min(MAX_NEW_PROJECTILES_PER_FRAME)) {
        cur_new_proj_ref[i] = Self::random_projectile(&self.random);
      }
    }

    // let output: ComputeOutput = unsafe { self.shader_output_data[frame_i].as_ref().clone() };
    // println!("output {:#?}", output);

    debug_assert!(output.new_projectile_count as usize <= PUSH_CONSTANT_NEXT_PROJECTILES_COUNT);

    self.constants.cur_projectile_count += output.new_projectile_count;

    let next_proj_i = output.pc_next_projectiles_i;
    for i in 0..((next_proj_i as usize).min(PUSH_CONSTANT_NEXT_PROJECTILES_COUNT)) {
      let new_proj = Projectile {
        pos: [(self.random.gen::<f32>() - 0.5) * 2.0, -1.0],
        vel: [0.0, 0.2],
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
