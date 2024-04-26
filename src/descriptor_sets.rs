use std::{
  marker::PhantomData,
  ptr::{self, addr_of},
};

use ash::vk;

use crate::{device_destroyable::DeviceManuallyDestroyed, errors::OutOfMemoryError};

pub struct DescriptorSets {
  pub layout: vk::DescriptorSetLayout,
  pool: vk::DescriptorPool,
  pub mandelbrot_image: vk::DescriptorSet,
}

impl DescriptorSets {
  pub fn new(device: &ash::Device) -> Result<Self, OutOfMemoryError> {
    let layout = create_layout(device)?;
    let pool = create_pool(device)?;

    let allocate_info = vk::DescriptorSetAllocateInfo {
      s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
      p_next: ptr::null(),
      descriptor_pool: pool,
      descriptor_set_count: 1,
      p_set_layouts: addr_of!(layout),
      _marker: PhantomData,
    };
    let mandelbrot_image = unsafe { device.allocate_descriptor_sets(&allocate_info)?[0] };

    Ok(Self {
      layout,
      pool,
      mandelbrot_image,
    })
  }

  pub fn write_image(&mut self, device: &ash::Device, view: vk::ImageView) {
    let image_info = vk::DescriptorImageInfo {
      sampler: vk::Sampler::null(),
      image_view: view,
      image_layout: vk::ImageLayout::GENERAL, // required for use as storage
    };

    let write = vk::WriteDescriptorSet {
      s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
      p_next: ptr::null(),
      dst_set: self.mandelbrot_image,
      dst_binding: 0,
      dst_array_element: 0,
      descriptor_count: 1,
      descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
      p_buffer_info: ptr::null(),
      p_image_info: addr_of!(image_info),
      p_texel_buffer_view: ptr::null(),
      _marker: PhantomData,
    };

    unsafe {
      device.update_descriptor_sets(&[write], &[]);
    }
  }
}

fn create_layout(device: &ash::Device) -> Result<vk::DescriptorSetLayout, OutOfMemoryError> {
  let bindings = [vk::DescriptorSetLayoutBinding {
    binding: 0,
    descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
    descriptor_count: 1,
    stage_flags: vk::ShaderStageFlags::COMPUTE,
    p_immutable_samplers: ptr::null(),
    _marker: PhantomData,
  }];

  let create_info = vk::DescriptorSetLayoutCreateInfo {
    s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::DescriptorSetLayoutCreateFlags::empty(),
    binding_count: bindings.len() as u32,
    p_bindings: bindings.as_ptr(),
    _marker: PhantomData,
  };

  unsafe {
    device
      .create_descriptor_set_layout(&create_info, None)
      .map_err(|err| err.into())
  }
}

fn create_pool(device: &ash::Device) -> Result<vk::DescriptorPool, OutOfMemoryError> {
  let sizes = [vk::DescriptorPoolSize {
    ty: vk::DescriptorType::STORAGE_IMAGE,
    descriptor_count: 1,
  }];
  let pool_create_info = vk::DescriptorPoolCreateInfo {
    s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
    p_next: ptr::null(),
    pool_size_count: sizes.len() as u32,
    p_pool_sizes: sizes.as_ptr(),
    max_sets: 1,
    flags: vk::DescriptorPoolCreateFlags::empty(),
    _marker: PhantomData,
  };
  unsafe { device.create_descriptor_pool(&pool_create_info, None) }.map_err(|err| err.into())
}

impl DeviceManuallyDestroyed for DescriptorSets {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_descriptor_pool(self.pool, None);
    device.destroy_descriptor_set_layout(self.layout, None);
  }
}
