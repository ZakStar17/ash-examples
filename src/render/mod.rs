mod allocator;
mod command_pools;
mod create_objs;
mod data;
mod descriptor_sets;
mod device_destroyable;
mod errors;
mod initialization;
mod pipelines;
mod render_object;
mod render_pass;
mod render_targets;
mod renderer;
mod shaders;
mod swapchain;
mod sync_renderer;
mod vertices;

use ash::vk;
use std::ffi::CStr;

pub use errors::{FrameRenderError, InitializationError};
pub use initialization::{RenderInit, RenderInitError};
pub use render_object::RenderPosition;
pub use swapchain::AcquireNextImageError;
pub use sync_renderer::SyncRenderer;

use crate::utility::const_flag_bitor;

const FRAMES_IN_FLIGHT: usize = 2;

// validation layers names should be valid cstrings (not contain null bytes nor invalid characters)
#[cfg(feature = "vl")]
const VALIDATION_LAYERS: [&CStr; 1] = [c"VK_LAYER_KHRONOS_validation"];
#[cfg(feature = "vl")]
const ADDITIONAL_VALIDATION_FEATURES: [vk::ValidationFeatureEnableEXT; 2] = [
  vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
  vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
];

const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

const RENDER_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;

const SWAPCHAIN_IMAGE_USAGES: vk::ImageUsageFlags = const_flag_bitor!(vk::ImageUsageFlags => vk::ImageUsageFlags::COLOR_ATTACHMENT, vk::ImageUsageFlags::TRANSFER_DST);
