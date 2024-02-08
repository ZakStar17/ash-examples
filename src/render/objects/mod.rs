pub mod command_pools;
mod constant_buffers;
mod descriptor_sets;
pub mod device;
mod entry;
mod instance;
mod pipeline;
mod pipeline_cache;
mod render_pass;
mod surface;
mod swapchain;

#[cfg(feature = "vl")]
mod validation_layers;

use std::ptr;

use ash::vk;

pub use constant_buffers::{allocate_and_bind_memory_to_buffers, create_buffer, ConstantBuffers};
pub use descriptor_sets::DescriptorSets;
pub use entry::get_entry;
pub use instance::create_instance;
pub use pipeline::GraphicsPipeline;
pub use pipeline_cache::{create_pipeline_cache, save_pipeline_cache};
pub use render_pass::{create_framebuffer, create_render_pass};
pub use surface::Surface;
pub use swapchain::Swapchains;

#[cfg(feature = "vl")]
pub use validation_layers::{get_supported_validation_layers, DebugUtils};

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
