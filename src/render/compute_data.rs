use std::{
  mem::{offset_of, size_of},
  ops::BitOr,
  ptr::{self, NonNull},
};

use ash::vk;
use rand::{rngs::ThreadRng, Rng};

use crate::{
  render::{allocations::allocate_and_bind_memory, common_object_creations::create_buffer},
  utility,
};

use super::{
  command_pools::compute::{AddNewBullets, ComputeRecordBufferData, ExecuteShader},
  descriptor_sets::{storage_buffer_descriptor_set, BufferWriteDescriptorSet, DescriptorPool},
  initialization::device::PhysicalDevice,
  FRAMES_IN_FLIGHT,
};

pub const PUSH_CONSTANT_PROJECTILE_REPLACEMENTS_COUNT: usize = 8;
pub const MAX_NEW_PROJECTILES_PER_FRAME: usize = 1024;

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
  pub delta_time: f32,

  pub bullet_count: u32, // size: 4
  pub bullet_replacements: [Bullet; PUSH_CONSTANT_PROJECTILE_REPLACEMENTS_COUNT],
}

// host accessible data after shader dispatch
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct ComputeOutput {
  colliding: u32,
  // number of bullets replaced by that shader dispatch
  pc_bullet_replacements_i: u32,
}

#[derive(Debug)]
pub struct MappedHostBuffer<T> {
  pub buffer: vk::Buffer,
  pub data_ptr: NonNull<T>,
}

#[derive(Debug)]
pub struct ComputeData {
  pub host_memory: vk::DeviceMemory,
  pub output: [MappedHostBuffer<ComputeOutput>; FRAMES_IN_FLIGHT],
  pub new_bullets: [MappedHostBuffer<[Bullet; MAX_NEW_PROJECTILES_PER_FRAME]>; FRAMES_IN_FLIGHT],

  pub device_memory: vk::DeviceMemory,
  pub instance_capacity: u64,
  pub instance_compute: [vk::Buffer; FRAMES_IN_FLIGHT],
  pub instance_graphics: [vk::Buffer; FRAMES_IN_FLIGHT],

  pub descriptor_sets: [vk::DescriptorSet; FRAMES_IN_FLIGHT],

  target_bullet_count: usize,
  cur_bullet_count: usize,
  rng: ThreadRng,
  bullet_replacements_cache: [[Bullet; PUSH_CONSTANT_PROJECTILE_REPLACEMENTS_COUNT]; 2],
}

impl ComputeData {
  pub const STORAGE_BUFFERS_IN_SETS: u32 = 4;

  pub fn new(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    descriptor_pool: &mut DescriptorPool,
  ) -> Self {
    let new_bullets_size = size_of::<Bullet>() * MAX_NEW_PROJECTILES_PER_FRAME;
    let shader_output = utility::populate_array_with_expression!(
      create_buffer(
        device,
        size_of::<ComputeOutput>() as u64,
        // transfer dst is used in a buffer clear command
        vk::BufferUsageFlags::STORAGE_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      ),
      FRAMES_IN_FLIGHT
    );
    let new_bullets = utility::populate_array_with_expression!(
      create_buffer(
        device,
        new_bullets_size as u64,
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
      &[
        shader_output[0],
        shader_output[1],
        new_bullets[0],
        new_bullets[1],
      ],
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
    let shader_output_ptrs = unsafe {
      [
        NonNull::new_unchecked(host_ptr.add(offsets[0] as usize) as *mut ComputeOutput),
        NonNull::new_unchecked(host_ptr.add(offsets[1] as usize) as *mut ComputeOutput),
      ]
    };
    let new_bullets_ptrs = unsafe {
      [
        NonNull::new_unchecked(
          host_ptr.add(offsets[2] as usize) as *mut [Bullet; MAX_NEW_PROJECTILES_PER_FRAME]
        ),
        NonNull::new_unchecked(
          host_ptr.add(offsets[3] as usize) as *mut [Bullet; MAX_NEW_PROJECTILES_PER_FRAME]
        ),
      ]
    };

    let initial_capacity = 40000;
    let instance_size = (size_of::<Bullet>() * initial_capacity) as u64;
    let instance_compute = utility::populate_array_with_expression!(
      create_buffer(
        device,
        instance_size,
        vk::BufferUsageFlags::STORAGE_BUFFER
          .bitor(vk::BufferUsageFlags::TRANSFER_DST)
          .bitor(vk::BufferUsageFlags::TRANSFER_SRC),
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
      &[
        instance_compute[0],
        instance_compute[1],
        instance_graphics[0],
        instance_graphics[1],
      ],
      &[],
    )
    .unwrap();

    let layouts = [
      descriptor_pool.compute_layout,
      descriptor_pool.compute_layout,
    ];
    let descriptor_sets = descriptor_pool.allocate_sets(device, &layouts).unwrap();
    let descriptor_sets = [descriptor_sets[0], descriptor_sets[1]];

    let mut rng = rand::thread_rng();
    let mut bullet_replacements_cache =
      [[Bullet::default(); PUSH_CONSTANT_PROJECTILE_REPLACEMENTS_COUNT]; 2];
    for i in 0..PUSH_CONSTANT_PROJECTILE_REPLACEMENTS_COUNT {
      bullet_replacements_cache[0][i] = Self::random_bullet(&mut rng);
    }
    for i in 0..PUSH_CONSTANT_PROJECTILE_REPLACEMENTS_COUNT {
      bullet_replacements_cache[1][i] = Self::random_bullet(&mut rng);
    }

    Self {
      host_memory: host_alloc.memory,
      output: [
        MappedHostBuffer {
          buffer: shader_output[0],
          data_ptr: shader_output_ptrs[0],
        },
        MappedHostBuffer {
          buffer: shader_output[1],
          data_ptr: shader_output_ptrs[1],
        },
      ],
      new_bullets: [
        MappedHostBuffer {
          buffer: new_bullets[0],
          data_ptr: new_bullets_ptrs[0],
        },
        MappedHostBuffer {
          buffer: new_bullets[1],
          data_ptr: new_bullets_ptrs[1],
        },
      ],

      device_memory: device_alloc.memory,
      instance_compute,
      instance_graphics,

      descriptor_sets,

      instance_capacity: initial_capacity as u64,
      target_bullet_count: 40000,
      cur_bullet_count: 0,
      rng,
      bullet_replacements_cache,
    }
  }

  fn random_bullet(rng: &mut ThreadRng) -> Bullet {
    Bullet {
      pos: [(rng.gen::<f32>() - 0.5) * 2.2, -1.2],
      vel: [0.0, 0.1 + (rng.gen::<f32>() / 2.0)],
    }
  }

  pub fn update(
    &mut self,
    frame_i: usize,
    shader_completed_last_frame: bool,
    delta_time: f32,
    player_position: [f32; 2],
  ) -> (ComputeRecordBufferData, usize) {
    if shader_completed_last_frame {
      let output = unsafe { self.output[frame_i].data_ptr.as_ref().clone() };

      for i in 0..(output.pc_bullet_replacements_i as usize)
        .min(PUSH_CONSTANT_PROJECTILE_REPLACEMENTS_COUNT)
      {
        self.bullet_replacements_cache[frame_i][i] = Self::random_bullet(&mut self.rng);
      }
    }

    let before_adding_count = self.cur_bullet_count;
    let mut total_count = self.cur_bullet_count;

    let execute_shader = if before_adding_count > 0 {
      Some(ExecuteShader {
        push_data: ComputePushConstants {
          player_pos: player_position,
          delta_time,
          bullet_count: before_adding_count as u32, // before adding more bullets
          bullet_replacements: self.bullet_replacements_cache[frame_i],
        },
      })
    } else {
      None
    };

    let add_bullets = if self.target_bullet_count > before_adding_count {
      let cur_new_proj_ref = unsafe { self.new_bullets[frame_i].data_ptr.as_mut() };
      let new_bullet_count =
        (self.target_bullet_count - before_adding_count).min(MAX_NEW_PROJECTILES_PER_FRAME);
      for i in 0..new_bullet_count {
        cur_new_proj_ref[i] = Self::random_bullet(&mut self.rng);
      }

      self.cur_bullet_count += new_bullet_count;
      total_count += new_bullet_count;

      Some(AddNewBullets {
        buffer: self.new_bullets[frame_i].buffer,
        buffer_size: (size_of::<Bullet>() * new_bullet_count) as u64,
        bullet_count: new_bullet_count,
      })
    } else {
      None
    };

    (
      ComputeRecordBufferData {
        output: self.output[frame_i].buffer,
        instance_read: self.instance_compute[(frame_i + 1) % FRAMES_IN_FLIGHT],
        instance_write: self.instance_compute[frame_i],
        instance_graphics: self.instance_graphics[frame_i],
        existing_bullets_count: before_adding_count,
        add_bullets,
        execute_shader,
      },
      total_count,
    )
  }

  pub fn get_set_writes(&self) -> ([BufferWriteDescriptorSet; 4], [vk::CopyDescriptorSet; 2]) {
    let writes = [
      storage_buffer_descriptor_set(
        self.descriptor_sets[0],
        0,
        vk::DescriptorBufferInfo {
          buffer: self.output[0].buffer,
          offset: 0,
          range: vk::WHOLE_SIZE,
        },
      ),
      storage_buffer_descriptor_set(
        self.descriptor_sets[0],
        1,
        vk::DescriptorBufferInfo {
          buffer: self.instance_compute[1],
          offset: 0,
          range: vk::WHOLE_SIZE,
        },
      ),
      storage_buffer_descriptor_set(
        self.descriptor_sets[0],
        2,
        vk::DescriptorBufferInfo {
          buffer: self.instance_compute[0],
          offset: 0,
          range: vk::WHOLE_SIZE,
        },
      ),
      storage_buffer_descriptor_set(
        self.descriptor_sets[1],
        0,
        vk::DescriptorBufferInfo {
          buffer: self.output[1].buffer,
          offset: 0,
          range: vk::WHOLE_SIZE,
        },
      ),
    ];

    let from_first_to_second_copy_base = vk::CopyDescriptorSet {
      s_type: vk::StructureType::COPY_DESCRIPTOR_SET,
      p_next: ptr::null(),
      src_set: self.descriptor_sets[0],
      src_binding: 0,
      src_array_element: 0,
      dst_set: self.descriptor_sets[1],
      dst_binding: 0,
      dst_array_element: 0,
      descriptor_count: 1,
    };
    // second set has instance_compute buffers reverted
    let copies = [
      vk::CopyDescriptorSet {
        src_binding: 1,
        dst_binding: 2,
        ..from_first_to_second_copy_base
      },
      vk::CopyDescriptorSet {
        src_binding: 2,
        dst_binding: 1,
        ..from_first_to_second_copy_base
      },
    ];

    (writes, copies)
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    for i in 0..FRAMES_IN_FLIGHT {
      device.destroy_buffer(self.output[i].buffer, None);
      device.destroy_buffer(self.new_bullets[i].buffer, None);
      device.destroy_buffer(self.instance_compute[i], None);
      device.destroy_buffer(self.instance_graphics[i], None);
    }
    device.free_memory(self.host_memory, None);
    device.free_memory(self.device_memory, None);
  }
}
