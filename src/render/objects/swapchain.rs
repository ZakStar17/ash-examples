use std::{ops::Deref, ptr};

pub use ash::vk;
use winit::dpi::PhysicalSize;

use crate::PREFERRED_PRESENTATION_METHOD;

use super::{
  create_image_view,
  device::{PhysicalDevice, QueueFamilies},
  Surface,
};

pub struct Swapchains {
  loader: ash::extensions::khr::Swapchain,
  current: Swapchain,
  old: Option<Swapchain>,
}

impl Swapchains {
  pub fn new(
    instance: &ash::Instance,
    physical_device: &PhysicalDevice,
    device: &ash::Device,
    surface: &Surface,
    window_size: PhysicalSize<u32>,
  ) -> Self {
    let loader = ash::extensions::khr::Swapchain::new(instance, device);

    let current = Swapchain::create(physical_device, device, surface, &loader, window_size);

    Self {
      loader,
      current,
      old: None,
    }
  }

  pub unsafe fn acquire_next_image(
    &mut self,
    semaphore: vk::Semaphore,
  ) -> Result<(u32, bool), vk::Result> {
    self.current.acquire_next_image(semaphore, &self.loader)
  }

  pub unsafe fn recreate_swapchain(
    &mut self,
    physical_device: &PhysicalDevice,
    device: &ash::Device,
    surface: &Surface,
    window_size: PhysicalSize<u32>,
  ) -> RecreationChanges {
    let (old, changes) =
      self
        .current
        .recreate(physical_device, device, surface, &self.loader, window_size);

    self.old = Some(old);
    changes
  }

  pub unsafe fn queue_present(
    &mut self,
    image_index: u32,
    present_queue: vk::Queue,
    wait_semaphores: &[vk::Semaphore],
  ) -> Result<bool, vk::Result> {
    let present_info = vk::PresentInfoKHR {
      s_type: vk::StructureType::PRESENT_INFO_KHR,
      p_next: ptr::null(),
      wait_semaphore_count: wait_semaphores.len() as u32,
      p_wait_semaphores: wait_semaphores.as_ptr(),
      swapchain_count: 1,
      p_swapchains: &*self.current,
      p_image_indices: &image_index,
      p_results: ptr::null_mut(),
    };

    unsafe { self.loader.queue_present(present_queue, &present_info) }
  }

  pub fn destroy_old(&mut self, device: &ash::Device) {
    if let Some(old) = &mut self.old {
      unsafe {
        old.destroy_self(device, &self.loader);
      }
      self.old = None;
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    self.destroy_old(device);
    self.current.destroy_self(device, &self.loader);
  }

  pub fn get_format(&self) -> vk::Format {
    self.current.format
  }

  pub fn get_extent(&self) -> vk::Extent2D {
    self.current.extent
  }

  pub fn get_image_views(&self) -> &[vk::ImageView] {
    &self.current.image_views
  }
}

#[derive(Debug)]
struct Swapchain {
  vk_obj: vk::SwapchainKHR,
  _images: Box<[vk::Image]>, // are owned by the swapchain
  pub format: vk::Format,
  pub extent: vk::Extent2D,
  pub image_views: Box<[vk::ImageView]>,
}

impl Deref for Swapchain {
  type Target = vk::SwapchainKHR;

  fn deref(&self) -> &Self::Target {
    &self.vk_obj
  }
}

pub struct RecreationChanges {
  pub format: bool,
  pub extent: bool,
}

impl Swapchain {
  pub fn create(
    physical_device: &PhysicalDevice,
    device: &ash::Device,
    surface: &Surface,
    swapchain_loader: &ash::extensions::khr::Swapchain,
    window_size: PhysicalSize<u32>,
  ) -> Self {
    let capabilities = unsafe { surface.get_capabilities(**physical_device) };
    let image_format = select_swapchain_image_format(**physical_device, surface);
    let present_mode = select_swapchain_present_mode(**physical_device, surface);
    let extent = get_swapchain_extent(&capabilities, window_size);

    log::info!(
      "Creating swapchain with ({}, {}) extent, {:?} format and {:?} present mode",
      extent.width,
      extent.height,
      image_format,
      present_mode
    );

    Self::create_with(
      device,
      &physical_device.queue_families,
      surface,
      swapchain_loader,
      capabilities,
      image_format,
      present_mode,
      extent,
      vk::SwapchainKHR::null(),
    )
  }

  pub fn recreate(
    &mut self,
    physical_device: &PhysicalDevice,
    device: &ash::Device,
    surface: &Surface,
    swapchain_loader: &ash::extensions::khr::Swapchain,
    window_size: PhysicalSize<u32>,
  ) -> (Self, RecreationChanges) {
    let capabilities = unsafe { surface.get_capabilities(**physical_device) };
    let image_format = select_swapchain_image_format(**physical_device, surface);
    let present_mode = select_swapchain_present_mode(**physical_device, surface);
    let extent = get_swapchain_extent(&capabilities, window_size);

    log::info!(
      "Recreating swapchain with ({}, {}) extent, {:?} format and {:?} present mode",
      extent.width,
      extent.height,
      image_format,
      present_mode
    );

    let changes = RecreationChanges {
      format: image_format.format != self.format,
      extent: extent != self.extent,
    };

    let mut new = Self::create_with(
      device,
      &physical_device.queue_families,
      surface,
      swapchain_loader,
      capabilities,
      image_format,
      present_mode,
      extent,
      self.vk_obj,
    );

    let old = {
      std::mem::swap(&mut new, self);
      new
    };

    (old, changes)
  }

  fn create_with(
    device: &ash::Device,
    queue_families: &QueueFamilies,
    surface: &Surface,
    swapchain_loader: &ash::extensions::khr::Swapchain,
    capabilities: vk::SurfaceCapabilitiesKHR,
    image_format: vk::SurfaceFormatKHR,
    present_mode: vk::PresentModeKHR,
    extent: vk::Extent2D,
    old_swapchain: vk::SwapchainKHR,
  ) -> Self {
    let image_count = if capabilities.max_image_count > 0 {
      (capabilities.min_image_count + 1).min(capabilities.max_image_count)
    } else {
      capabilities.min_image_count + 1
    };

    let mut create_info = vk::SwapchainCreateInfoKHR {
      s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
      p_next: ptr::null(),
      flags: vk::SwapchainCreateFlagsKHR::empty(),
      surface: **surface,

      min_image_count: image_count,
      image_color_space: image_format.color_space,
      image_format: image_format.format,
      image_extent: extent,
      image_array_layers: 1,
      image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,

      image_sharing_mode: vk::SharingMode::EXCLUSIVE,
      // ignored when SharingMode is EXCLUSIVE
      p_queue_family_indices: ptr::null(),
      queue_family_index_count: 0,

      pre_transform: capabilities.current_transform,
      composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
      present_mode,
      clipped: vk::TRUE,
      old_swapchain,
    };

    // in rare cases that presentation != graphics, set sharing mode to CONCURRENT with both
    // families
    let _family_indices =
      if queue_families.get_graphics_index() != queue_families.get_presentation_index() {
        let family_indices = [
          queue_families.get_graphics_index(),
          queue_families.get_presentation_index(),
        ];
        create_info.image_sharing_mode = vk::SharingMode::CONCURRENT;
        create_info.p_queue_family_indices = family_indices.as_ptr();
        create_info.queue_family_index_count = family_indices.len() as u32;

        Some(family_indices)
      } else {
        None
      };

    let swapchain = unsafe {
      swapchain_loader
        .create_swapchain(&create_info, None)
        .expect("Failed to create Swapchain!")
    };

    let images = unsafe {
      swapchain_loader
        .get_swapchain_images(swapchain)
        .expect("Failed to get Swapchain Images.")
        .into_boxed_slice()
    };
    let image_views = images
      .iter()
      .map(|&image| create_image_view(device, image, image_format.format))
      .collect();

    Self {
      vk_obj: swapchain,
      _images: images,
      format: image_format.format,
      extent,
      image_views,
    }
  }

  pub unsafe fn acquire_next_image(
    &mut self,
    semaphore: vk::Semaphore,
    loader: &ash::extensions::khr::Swapchain,
  ) -> Result<(u32, bool), vk::Result> {
    loader.acquire_next_image(self.vk_obj, std::u64::MAX, semaphore, vk::Fence::null())
  }

  pub unsafe fn destroy_self(
    &mut self,
    device: &ash::Device,
    loader: &ash::extensions::khr::Swapchain,
  ) {
    for &view in self.image_views.iter() {
      device.destroy_image_view(view, None);
    }
    loader.destroy_swapchain(self.vk_obj, None);
  }
}

fn select_swapchain_image_format(
  physical_device: vk::PhysicalDevice,
  surface: &Surface,
) -> vk::SurfaceFormatKHR {
  let formats = unsafe { surface.get_formats(physical_device) };
  for available_format in formats.iter() {
    // commonly available
    if available_format.format == vk::Format::B8G8R8A8_SRGB
      && available_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
    {
      return *available_format;
    }
  }

  formats[0]
}

fn select_swapchain_present_mode(
  physical_device: vk::PhysicalDevice,
  surface: &Surface,
) -> vk::PresentModeKHR {
  let present_modes = unsafe { surface.get_present_modes(physical_device) };
  if present_modes.contains(&PREFERRED_PRESENTATION_METHOD) {
    return PREFERRED_PRESENTATION_METHOD;
  }

  if PREFERRED_PRESENTATION_METHOD == vk::PresentModeKHR::FIFO_RELAXED
    && present_modes.contains(&vk::PresentModeKHR::IMMEDIATE)
  {
    return vk::PresentModeKHR::IMMEDIATE;
  }

  // required to be available
  vk::PresentModeKHR::FIFO
}

fn get_swapchain_extent(
  capabilities: &vk::SurfaceCapabilitiesKHR,
  size: PhysicalSize<u32>,
) -> vk::Extent2D {
  match Surface::get_extent_from_capabilities(capabilities) {
    Some(extent) => extent,
    None => vk::Extent2D {
      width: size.width.clamp(
        capabilities.min_image_extent.width,
        capabilities.max_image_extent.width,
      ),
      height: size.height.clamp(
        capabilities.min_image_extent.height,
        capabilities.max_image_extent.height,
      ),
    },
  }
}
