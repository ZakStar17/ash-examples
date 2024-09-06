mod device;
mod host;

use std::{mem::offset_of, ops::Range};

use ash::vk;
use device::DeviceComputeData;
use host::HostComputeData;

use crate::render::{
  device_destroyable::DeviceManuallyDestroyed,
  errors::{AllocationError, OutOfMemoryError},
  initialization::device::{Device, PhysicalDevice},
  FRAMES_IN_FLIGHT,
};

// all data passed to the shader follows std430 layout rules
// https://www.oreilly.com/library/view/opengl-programming-guide/9780132748445/app09lev1sec3.html

// total number of possible new_projectiles each frame =
// (MAX_RANDOM_VALUES - COPY_RANDOM_THRESHOLD)
// divided by RANDOM_VALUES_USED_PER_BULLET

// that can be added each frame ()

// WARNING: should equal to what in the shader
//
// number of f32 in the "Random" uniform buffer
//
// the staging buffers could be smaller, but it would require some special handling for when the
// number of required random values exceeds what the staging buffer could handle
const MAX_RANDOM_VALUES: usize = 16384;

// do a cmd_copy_buffer to device memory when amount of used random values exceeds COPY_RANDOM_THRESHOLD
const COPY_RANDOM_THRESHOLD: usize = 512;

// ignore COPY_RANDOM_THRESHOLD and copy TO_END_OF_BUFFER_COPY_RANDOM_THRESHOLD or more values
// if shader started already wrapped around and started using values from the start
const TO_END_OF_BUFFER_COPY_RANDOM_THRESHOLD: usize = 256;

// according to shader
const RANDOM_VALUES_USED_PER_BULLET: usize = 2;

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
  pub player_pos: [f32; 2],

  pub bullet_count: u32,
  pub target_bullet_count: u32,
  pub random_uniform_reserved_index: u32,
}

// equal to src/render/shaders/compute/shader.comp
// see shader code for more information about fields
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct ComputeHostIO {
  // 1 if colliding with bullet, 0 otherwise
  pub colliding: u32,
  // this value can be in the range 0..(MAX_RANDOM_VALUES * 2).
  // see ComputeData::random_unrefreshed_start/end for explanation
  pub random_uniform_index: u32,
}

// Each index warps around and can be in the range 0..(MAX_RANDOM_VALUES * 2).
// The indexes go past MAX_RANDOM_VALUES just for simplicity to
// differentiate between the special case where start and end wrap to an equal value,
// that could mean that no unrefreshed values exist as well as all values being unrefreshed.
//
// This also helps if for some reason *_end overshoots and goes past *_start which still
// refreshes the values.
//
// examples (x is any value in the 0..(MAX_RANDOM_VALUES * 2) range):
//    start = 2
//    end = 10
//    M: Range 2..10 is unrefreshed
//
//    start = 10
//    end = MAX_RANDOM_VALUES + 2
//    M: Ranges 10..MAX_RANDOM_VALUES and 0..2 are unrefreshed
//
//    start = MAX_RANDOM_VALUES + 10
//    end = 2
//    M: Ranges 10..MAX_RANDOM_VALUES and 0..2 are unrefreshed
//
//    start = 10
//    end = 2
//    M: Some error occurred and shader used unrefreshed values.
//       All will be refreshed
//
//    start = x
//    end = x
//    M: No values are unrefreshed
//
//    start = 2
//    end = MAX_RANDOM_VALUES + 2
//    M: Special case: all values are unrefreshed
#[derive(Debug, Clone, Copy)]
struct RandomBufferUsedArea {
  start: usize,
  end: usize,
}

impl RandomBufferUsedArea {
  // end not inclusive
  // start in 0..2m range
  // end in 0..3m range
  fn raw_ranges(mut start: usize, mut end: usize) -> Option<(Range<usize>, Option<Range<usize>>)> {
    let m = MAX_RANDOM_VALUES;

    if start == end {
      return None;
    }

    let dist = end - start;
    if dist >= m {
      debug_assert!(dist == m);
      return Some((0..m, None));
    }

    if start >= m && end >= m {
      start -= m;
      end -= m;
    }

    return if start < m {
      if end <= m {
        Some((start..end, None))
      } else {
        Some((start..m, Some(0..(end - m))))
      }
    } else {
      // start >= m
      // end < m
      Some(((start - m)..m, Some(0..end)))
    };
  }
}

#[derive(Debug)]
pub struct ComputeData {
  pub host: HostComputeData,
  pub device: DeviceComputeData,

  bullet_count: usize,
  target_bullet_count: usize,

  random_used: [RandomBufferUsedArea; FRAMES_IN_FLIGHT],
}

pub struct ComputeDataUpdate {
  pub bullet_count: u32,
  pub target_bullet_count: u32,
  pub random_uniform_reserved_index: u32,
  pub copy_new_random_values: Option<(Range<usize>, Option<Range<usize>>)>,
  pub compute_io_updated: bool,
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
      random_used: [RandomBufferUsedArea {
        start: 0,
        end: MAX_RANDOM_VALUES,
      }; FRAMES_IN_FLIGHT],
    })
  }

  // safety: buffers should not be in use
  pub fn update(
    &mut self,
    frame_i: usize,
    device: &Device,
    physical_device: &PhysicalDevice,
  ) -> Result<ComputeDataUpdate, OutOfMemoryError> {
    let frame_used_area = self.random_used[frame_i];

    let last_frame_result = unsafe { self.host.read_compute_io(frame_i, device, physical_device) }?;
    let new_area_end = last_frame_result.random_uniform_index as usize; // new_area_end >= frame_used_area.start

    // refresh last frame written values in normal memory buffer
    let last_frame_ranges = RandomBufferUsedArea::raw_ranges(frame_used_area.end, new_area_end);
    if let Some((normal_range, wrapped_opt)) = last_frame_ranges {
      self.host.refresh_rng_buffer(frame_i, normal_range);
      if let Some(wrapped_range) = wrapped_opt {
        self.host.refresh_rng_buffer(frame_i, wrapped_range);
      }
    }

    self.random_used[frame_i].end = new_area_end % (MAX_RANDOM_VALUES * 2);

    let new_area_len = new_area_end - frame_used_area.start;
    let mut update_ranges = None;
    if new_area_len >= COPY_RANDOM_THRESHOLD {
      update_ranges = RandomBufferUsedArea::raw_ranges(frame_used_area.start, new_area_end);
      if let Some((normal_range, wrapped_opt)) = update_ranges.clone() {
        unsafe {
          self.host.copy_to_staging(frame_i, normal_range);
        }
        self.random_used[frame_i].start = self.random_used[frame_i].end; // set to 0 used values
        if let Some(wrapped_range) = wrapped_opt {
          unsafe {
            self.host.copy_to_staging(frame_i, wrapped_range);
          }
        }
      }
    } else if frame_used_area.start >= MAX_RANDOM_VALUES
      && new_area_end < MAX_RANDOM_VALUES
      && new_area_len >= TO_END_OF_BUFFER_COPY_RANDOM_THRESHOLD
    {
      let full_update_ranges =
        RandomBufferUsedArea::raw_ranges(frame_used_area.start, new_area_end);
      let (normal_range, _) = full_update_ranges.unwrap(); // wrapped ignored
      debug_assert!(normal_range.end == MAX_RANDOM_VALUES);

      update_ranges = Some((normal_range.clone(), None));

      self.random_used[frame_i].start = 0; // only wrapped_range values continue marked as used
      unsafe {
        self.host.copy_to_staging(frame_i, normal_range);
      }
    }

    let mut new_compute_io = last_frame_result;
    new_compute_io.random_uniform_index = self.random_used[frame_i].end as u32;

    if last_frame_result.colliding == 1 {
      println!("Colliding!");
      new_compute_io.colliding = 0;
    }

    let new_bullets = self.target_bullet_count - self.bullet_count;
    let reserved_index = new_compute_io.random_uniform_index;
    if new_bullets > 0 {
      new_compute_io.random_uniform_index += (new_bullets * RANDOM_VALUES_USED_PER_BULLET) as u32;
    }

    let update_compute_io = new_compute_io != last_frame_result;
    if update_compute_io {
      unsafe {
        self.host.write_compute_io(frame_i, new_compute_io);
      }
    }

    let update = ComputeDataUpdate {
      bullet_count: self.bullet_count as u32,
      target_bullet_count: self.target_bullet_count as u32,
      random_uniform_reserved_index: reserved_index,
      copy_new_random_values: update_ranges,
      compute_io_updated: update_compute_io,
    };
    self.bullet_count = self.target_bullet_count;

    Ok(update)
  }
}

impl DeviceManuallyDestroyed for ComputeData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.host.destroy_self(device);
    self.device.destroy_self(device);
  }
}
