pub mod allocations;
pub mod command_pools;
mod compute_pipeline;
mod constant_allocations;
mod descriptor_sets;
pub mod device;
mod entry;
mod instance;
mod pipeline_cache;
mod pipelines;
mod render_pass;
mod surface;
mod swapchain;

#[cfg(feature = "vl")]
mod validation_layers;

use std::ptr;

use ash::vk;

pub use compute_pipeline::ComputePipeline;
pub use constant_allocations::ConstantAllocatedObjects;
pub use descriptor_sets::DescriptorSets;
pub use entry::get_entry;
pub use instance::create_instance;
pub use pipeline_cache::{create_pipeline_cache, save_pipeline_cache};
pub use pipelines::Pipelines;
pub use render_pass::{create_framebuffer, create_render_pass};
pub use surface::Surface;
pub use swapchain::Swapchains;

#[cfg(feature = "vl")]
pub use validation_layers::{get_supported_validation_layers, DebugUtils};

// 1 color layer 2d image
pub fn create_image(
  device: &ash::Device,
  width: u32,
  height: u32,
  format: vk::Format,
  tiling: vk::ImageTiling,
  usage: vk::ImageUsageFlags,
) -> vk::Image {
  let create_info = vk::ImageCreateInfo {
    s_type: vk::StructureType::IMAGE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::ImageCreateFlags::empty(),
    image_type: vk::ImageType::TYPE_2D,
    format,
    extent: vk::Extent3D {
      width,
      height,
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

// 2d image all color channels
pub fn create_image_view(
  device: &ash::Device,
  image: vk::Image,
  format: vk::Format,
) -> vk::ImageView {
  let create_info = vk::ImageViewCreateInfo {
    s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::ImageViewCreateFlags::empty(),
    view_type: vk::ImageViewType::TYPE_2D,
    format,
    components: vk::ComponentMapping {
      r: vk::ComponentSwizzle::IDENTITY,
      g: vk::ComponentSwizzle::IDENTITY,
      b: vk::ComponentSwizzle::IDENTITY,
      a: vk::ComponentSwizzle::IDENTITY,
    },
    subresource_range: vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    },
    image,
  };

  unsafe {
    device
      .create_image_view(&create_info, None)
      .expect("Failed to create image view")
  }
}

pub fn create_semaphore(device: &ash::Device) -> vk::Semaphore {
  let semaphore_create_info = vk::SemaphoreCreateInfo {
    s_type: vk::StructureType::SEMAPHORE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::SemaphoreCreateFlags::empty(),
  };
  unsafe {
    device
      .create_semaphore(&semaphore_create_info, None)
      .expect("Failed to create semaphore")
  }
}

pub fn create_unsignaled_fence(device: &ash::Device) -> vk::Fence {
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
}
