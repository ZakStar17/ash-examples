use std::{
  ops::BitOr,
  ptr::{self, copy_nonoverlapping},
};

use ash::vk;
use image::ImageError;

use crate::render::objects::create_image_view;

use super::objects::{
  allocate_and_bind_memory_to_buffers,
  command_pools::{TemporaryGraphicsCommandBufferPool, TransferCommandBufferPool},
  create_buffer,
  device::{PhysicalDevice, Queues},
};

const IMAGE_PATH: &'static str = "./ferris.png";
pub const TEXTURE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;

pub struct Texture {
  memory: vk::DeviceMemory,
  image: vk::Image,
  pub image_view: vk::ImageView,
}

fn read_image_bytes() -> Result<(u32, u32, Vec<u8>), ImageError> {
  let img = image::io::Reader::open(IMAGE_PATH)?.decode()?.into_rgba8();
  let width = img.width();
  let height = img.height();

  let bytes = img.into_raw();
  assert!(bytes.len() == width as usize * height as usize * 4);
  Ok((width, height, bytes))
}

impl Texture {
  pub fn load(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    queues: &Queues,
    transfer_pool: &mut TransferCommandBufferPool,
    temp_graphics_pool: &mut TemporaryGraphicsCommandBufferPool,
  ) -> Self {
    let (width, height, bytes) = read_image_bytes().expect("Failed to load texture");
    log::debug!("Loaded texture with ({}, {}) dimensions", width, height);

    let staging_buffer = create_buffer(
      device,
      bytes.len() as u64,
      vk::BufferUsageFlags::TRANSFER_SRC,
    );
    let texture_image = create_image(
      device,
      width,
      height,
      vk::ImageTiling::OPTIMAL,
      vk::ImageUsageFlags::TRANSFER_DST.bitor(vk::ImageUsageFlags::SAMPLED),
    );

    let staging_buffer_alloc = allocate_and_bind_memory_to_buffers(
      device,
      physical_device,
      &[staging_buffer],
      vk::MemoryPropertyFlags::HOST_VISIBLE,
      vk::MemoryPropertyFlags::HOST_CACHED,
    );
    let (texture_memory, _memory_type, _memory_size) = allocate_and_bind_memory_to_image(
      device,
      physical_device,
      texture_image,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      vk::MemoryPropertyFlags::empty(),
    );

    unsafe {
      let mem_ptr = device
        .map_memory(
          staging_buffer_alloc.memory,
          0,
          staging_buffer_alloc.memory_size,
          vk::MemoryMapFlags::empty(),
        )
        .expect("Failed to map staging texture buffer memory") as *mut u8;

      copy_nonoverlapping(bytes.as_ptr(), mem_ptr, bytes.len());

      let mem_type = physical_device.get_memory_type(staging_buffer_alloc.memory_type);
      if !mem_type
        .property_flags
        .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
      {
        let range = vk::MappedMemoryRange {
          s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
          p_next: ptr::null(),
          memory: staging_buffer_alloc.memory,
          offset: 0,
          size: staging_buffer_alloc.memory_size,
        };
        device
          .flush_mapped_memory_ranges(&[range])
          .expect("Failed to flush host mapped staging texture buffer memory");
      }

      device.unmap_memory(staging_buffer_alloc.memory);
    }

    // record command buffers
    unsafe {
      transfer_pool.reset(device);
      temp_graphics_pool.reset(device);

      transfer_pool.record_load_texture(
        device,
        &physical_device.queue_families,
        staging_buffer,
        texture_image,
        width,
        height,
      );
      temp_graphics_pool.record_acquire_texture(
        device,
        &physical_device.queue_families,
        texture_image,
      );
    }

    let transfer_finished = {
      let semaphore_create_info = vk::SemaphoreCreateInfo {
        s_type: vk::StructureType::SEMAPHORE_CREATE_INFO,
        p_next: ptr::null(),
        flags: vk::SemaphoreCreateFlags::empty(),
      };
      unsafe {
        device
          .create_semaphore(&semaphore_create_info, None)
          .expect("Failed to create a semaphore")
      }
    };
    let graphics_acquire_finished = {
      let create_info = vk::FenceCreateInfo {
        s_type: vk::StructureType::FENCE_CREATE_INFO,
        p_next: ptr::null(),
        flags: vk::FenceCreateFlags::empty(),
      };
      unsafe {
        device
          .create_fence(&create_info, None)
          .expect("Failed to create fence")
      }
    };

    // submit command buffers
    let transfer_submit_info = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 0,
      p_wait_semaphores: ptr::null(),
      p_wait_dst_stage_mask: ptr::null(),
      command_buffer_count: 1,
      p_command_buffers: &transfer_pool.load_texture,
      signal_semaphore_count: 1,
      p_signal_semaphores: &transfer_finished,
    };
    let wait_for = vk::PipelineStageFlags::TRANSFER;
    let graphics_acquire_submit_info = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 1,
      p_wait_semaphores: &transfer_finished,
      p_wait_dst_stage_mask: &wait_for,
      command_buffer_count: 1,
      p_command_buffers: &temp_graphics_pool.acquire_texture,
      signal_semaphore_count: 0,
      p_signal_semaphores: ptr::null(),
    };
    unsafe {
      device
        .queue_submit(queues.transfer, &[transfer_submit_info], vk::Fence::null())
        .expect("Failed submit to queue");
      device
        .queue_submit(
          queues.graphics,
          &[graphics_acquire_submit_info],
          graphics_acquire_finished,
        )
        .expect("Failed submit to queue");
      device
        .wait_for_fences(&[graphics_acquire_finished], true, u64::MAX)
        .unwrap();
    }

    // destroy unused objects
    unsafe {
      device.destroy_fence(graphics_acquire_finished, None);
      device.destroy_semaphore(transfer_finished, None);

      device.destroy_buffer(staging_buffer, None);
      device.free_memory(staging_buffer_alloc.memory, None);
    }

    let image_view = create_image_view(device, texture_image, TEXTURE_FORMAT);

    Self {
      memory: texture_memory,
      image: texture_image,
      image_view,
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_image_view(self.image_view, None);
    device.destroy_image(self.image, None);
    device.free_memory(self.memory, None);
  }
}

fn create_image(
  device: &ash::Device,
  width: u32,
  height: u32,
  tiling: vk::ImageTiling,
  usage: vk::ImageUsageFlags,
) -> vk::Image {
  // 1 color layer 2d image
  let create_info = vk::ImageCreateInfo {
    s_type: vk::StructureType::IMAGE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::ImageCreateFlags::empty(),
    image_type: vk::ImageType::TYPE_2D,
    format: TEXTURE_FORMAT,
    extent: vk::Extent3D {
      width: width,
      height: height,
      depth: 1,
    },
    mip_levels: 1,
    array_layers: 1,
    samples: vk::SampleCountFlags::TYPE_1,
    tiling,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(), // ignored if sharing mode is exclusive
    initial_layout: vk::ImageLayout::UNDEFINED,
  };

  unsafe {
    device
      .create_image(&create_info, None)
      .expect("Failed to create image")
  }
}

// usually one allocation is used for multiple images
fn allocate_and_bind_memory_to_image(
  device: &ash::Device,
  physical_device: &PhysicalDevice,
  image: vk::Image,
  required_memory_properties: vk::MemoryPropertyFlags,
  optional_memory_properties: vk::MemoryPropertyFlags,
) -> (vk::DeviceMemory, u32, u64) {
  let memory_requirements = unsafe { device.get_image_memory_requirements(image) };

  if memory_requirements.size >= physical_device.get_max_memory_allocation_size() {
    panic!("Memory required to allocate an image ({}mb) is higher than the maximum allowed on the device ({}mb)", 
      memory_requirements.size / 1_000_000,
      physical_device.get_max_memory_allocation_size() / 1_000_000
    );
  }

  let memory_type = physical_device
    .find_optimal_memory_type(
      memory_requirements.memory_type_bits,
      required_memory_properties,
      optional_memory_properties,
    )
    .expect("Failed to find appropriate memory type for allocating an image");

  let heap_size = physical_device.get_memory_type_heap(memory_type).size;
  if memory_requirements.size >= heap_size {
    panic!("Memory required to allocate an image ({}mb) is higher than the size of the requested heap ({}mb)", 
      memory_requirements.size / 1_000_000,
      heap_size / 1_000_000
    );
  }

  let allocate_info = vk::MemoryAllocateInfo {
    s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
    p_next: ptr::null(),
    allocation_size: memory_requirements.size,
    memory_type_index: memory_type,
  };

  let memory = unsafe {
    device
      .allocate_memory(&allocate_info, None)
      .expect("Failed to allocate memory for an image")
  };

  unsafe {
    device
      .bind_image_memory(image, memory, 0)
      .expect("Failed to bind memory to image")
  };

  (memory, memory_type, memory_requirements.size)
}
