use std::{ops::BitOr, ptr};

use ash::vk;
use image::ImageError;
use winit::dpi::PhysicalSize;

use crate::{
  player_sprite::{self, SpritePushConstants},
  render::{
    objects::{
      allocations::allocate_and_bind_memory,
      command_pools::{
        GraphicsCommandBufferPool, TemporaryGraphicsCommandBufferPool, TransferCommandBufferPool,
      },
      create_image, create_image_view, create_pipeline_cache, DescriptorSets,
    },
    RENDER_FORMAT, TEXTURE_PATH,
  },
  utility::{self, populate_array_with_expression},
  RESOLUTION,
};

use super::{
  objects::{
    create_framebuffer, create_render_pass,
    device::{create_logical_device, PhysicalDevice, Queues},
    save_pipeline_cache, ConstantAllocatedObjects, Pipelines, Surface, Swapchains,
  },
  FRAMES_IN_FLIGHT,
};

const RENDER_EXTENT: vk::Extent2D = vk::Extent2D {
  width: RESOLUTION[0],
  height: RESOLUTION[1],
};

fn create_sampler(device: &ash::Device) -> vk::Sampler {
  let sampler_create_info = vk::SamplerCreateInfo {
    s_type: vk::StructureType::SAMPLER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::SamplerCreateFlags::empty(),
    mag_filter: vk::Filter::NEAREST,
    min_filter: vk::Filter::NEAREST,
    address_mode_u: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    address_mode_v: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    address_mode_w: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    anisotropy_enable: vk::FALSE,
    max_anisotropy: 0.0,
    border_color: vk::BorderColor::INT_OPAQUE_BLACK,
    unnormalized_coordinates: vk::TRUE,
    compare_enable: vk::FALSE,
    compare_op: vk::CompareOp::NEVER,
    mipmap_mode: vk::SamplerMipmapMode::NEAREST,
    mip_lod_bias: 0.0,
    max_lod: 0.0,
    min_lod: 0.0,
  };
  unsafe {
    device
      .create_sampler(&sampler_create_info, None)
      .expect("Failed to create a sampler")
  }
}

fn read_texture_bytes_as_rgba8() -> Result<(u32, u32, Vec<u8>), ImageError> {
  let img = image::io::Reader::open(TEXTURE_PATH)?
    .decode()?
    .into_rgba8();
  let width = img.width();
  let height = img.height();

  let bytes = img.into_raw();
  assert!(bytes.len() == width as usize * height as usize * 4);
  Ok((width, height, bytes))
}

pub struct Renderer {
  pub physical_device: PhysicalDevice,
  pub device: ash::Device,
  pub queues: Queues,

  pub swapchains: Swapchains,

  render_pass: vk::RenderPass,
  // images to be rendered to
  render_targets: [vk::Image; FRAMES_IN_FLIGHT],
  render_targets_memory: vk::DeviceMemory,
  render_target_views: [vk::ImageView; FRAMES_IN_FLIGHT],
  framebuffers: [vk::Framebuffer; FRAMES_IN_FLIGHT],

  descriptor_sets: DescriptorSets,
  pipeline_cache: vk::PipelineCache,
  pipelines: Pipelines,

  pub graphics_pools: [GraphicsCommandBufferPool; FRAMES_IN_FLIGHT],
  constant_objects: ConstantAllocatedObjects,
  sampler: vk::Sampler,
}

impl Renderer {
  pub fn new(
    instance: &ash::Instance,
    surface: &Surface,
    initial_window_size: PhysicalSize<u32>,
  ) -> Self {
    let physical_device = unsafe { PhysicalDevice::select(&instance, surface) };
    let (device, queues) = create_logical_device(&instance, &physical_device);

    let render_pass = create_render_pass(&device, RENDER_FORMAT);
    let render_targets = utility::populate_array_with_expression!(
      create_image(
        &device,
        RESOLUTION[0],
        RESOLUTION[1],
        RENDER_FORMAT,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::COLOR_ATTACHMENT.bitor(vk::ImageUsageFlags::TRANSFER_SRC)
      ),
      2
    );
    let render_targets_memory = {
      let allocation = allocate_and_bind_memory(
        &device,
        &physical_device,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        vk::MemoryPropertyFlags::empty(),
        &[],
        &render_targets,
      )
      .expect("Failed to allocate main render target images");
      allocation.memory
    };
    let render_target_views = {
      let mut temp = [vk::ImageView::null(); 2];
      temp[0] = create_image_view(&device, render_targets[0], RENDER_FORMAT);
      temp[1] = create_image_view(&device, render_targets[1], RENDER_FORMAT);
      temp
    };
    let framebuffers = {
      let mut temp = [vk::Framebuffer::null(); 2];
      temp[0] = create_framebuffer(&device, render_pass, render_target_views[0], RENDER_EXTENT);
      temp[1] = create_framebuffer(&device, render_pass, render_target_views[1], RENDER_EXTENT);
      temp
    };

    let mut descriptor_sets = DescriptorSets::new(&device);

    log::info!("Creating pipeline cache");
    let (pipeline_cache, created_from_file) = create_pipeline_cache(&device, &physical_device);
    if created_from_file {
      log::info!("Cache successfully created from an existing cache file");
    } else {
      log::info!("Cache initialized as empty");
    }
    let pipelines = Pipelines::new(
      &device,
      pipeline_cache,
      render_pass,
      &descriptor_sets,
      RENDER_EXTENT,
    );

    let constant_objects = {
      let mut transfer_pool =
        TransferCommandBufferPool::create(&device, &physical_device.queue_families);
      let mut graphics_pool =
        TemporaryGraphicsCommandBufferPool::create(&device, &physical_device.queue_families);

      let (texture_width, texture_height, texture_bytes) =
        read_texture_bytes_as_rgba8().expect("Failed to read texture file");

      let objects = ConstantAllocatedObjects::new(
        &device,
        &physical_device,
        &queues,
        &mut transfer_pool,
        &mut graphics_pool,
        &player_sprite::VERTICES,
        &player_sprite::INDICES,
        &texture_bytes,
        texture_width,
        texture_height,
      );

      unsafe {
        transfer_pool.destroy_self(&device);
        graphics_pool.destroy_self(&device);
      }

      objects
    };

    let sampler = create_sampler(&device);
    descriptor_sets
      .pool
      .write_texture(&device, constant_objects.texture_view, sampler);

    let graphics_pools = populate_array_with_expression!(
      GraphicsCommandBufferPool::create(&device, &physical_device.queue_families),
      FRAMES_IN_FLIGHT
    );

    let swapchains = Swapchains::new(
      instance,
      &physical_device,
      &device,
      surface,
      initial_window_size,
    );

    Self {
      physical_device,
      device,
      queues,

      swapchains,

      render_pass,
      render_targets,
      render_targets_memory,
      render_target_views,
      framebuffers,

      descriptor_sets,
      pipeline_cache,
      pipelines,

      graphics_pools,
      constant_objects,
      sampler,
    }
  }

  pub unsafe fn record_graphics(
    &mut self,
    frame_i: usize,
    image_i: usize,
    player: &SpritePushConstants,
  ) {
    self.graphics_pools[frame_i].record(
      &self.device,
      self.render_pass,
      self.render_targets[frame_i],
      self.framebuffers[frame_i],
      RENDER_EXTENT,
      self.swapchains.get_images()[image_i],
      self.swapchains.get_extent(),
      &self.descriptor_sets,
      &self.pipelines,
      &self.constant_objects,
      player,
    );
  }

  pub unsafe fn recreate_swapchain(&mut self, surface: &Surface, window_size: PhysicalSize<u32>) {
    // old swapchain becomes retired
    let changes =
      self
        .swapchains
        .recreate_swapchain(&self.physical_device, &self.device, surface, window_size);

    if !changes.format && !changes.extent {
      log::warn!("Recreated swapchain without any extent or format change");
    }
  }

  // destroy old objects that resulted of a swapchain recreation
  // this should only be called when they stop being in use
  pub unsafe fn destroy_old(&mut self) {
    self.swapchains.destroy_old(&self.device);
  }

  pub unsafe fn destroy_self(&mut self) {
    self.device.destroy_sampler(self.sampler, None);
    self.constant_objects.destroy_self(&self.device);
    for pool in self.graphics_pools.iter_mut() {
      pool.destroy_self(&self.device);
    }

    log::info!("Saving pipeline cache");
    if let Err(err) = save_pipeline_cache(&self.device, &self.physical_device, self.pipeline_cache)
    {
      log::error!("Failed to save pipeline cache: {:?}", err);
    }
    self
      .device
      .destroy_pipeline_cache(self.pipeline_cache, None);

    self.pipelines.destroy_self(&self.device);

    self.descriptor_sets.destroy_self(&self.device);

    for &framebuffer in self.framebuffers.iter() {
      self.device.destroy_framebuffer(framebuffer, None);
    }
    for &view in self.render_target_views.iter() {
      self.device.destroy_image_view(view, None);
    }
    for &image in self.render_targets.iter() {
      self.device.destroy_image(image, None);
    }
    self.device.free_memory(self.render_targets_memory, None);

    self.device.destroy_render_pass(self.render_pass, None);

    self.swapchains.destroy_self(&self.device);

    self.device.destroy_device(None);
  }
}
