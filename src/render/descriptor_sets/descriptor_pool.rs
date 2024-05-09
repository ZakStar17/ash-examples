use std::{marker::PhantomData, ptr};

use ash::vk;

use crate::render::{device_destroyable::DeviceManuallyDestroyed, errors::OutOfMemoryError};

fn create_texture_sampler(device: &ash::Device) -> Result<vk::Sampler, OutOfMemoryError> {
  let sampler_create_info = vk::SamplerCreateInfo {
    s_type: vk::StructureType::SAMPLER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::SamplerCreateFlags::empty(),
    mag_filter: vk::Filter::LINEAR,
    min_filter: vk::Filter::LINEAR,
    address_mode_u: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    address_mode_v: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    address_mode_w: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    anisotropy_enable: vk::FALSE,
    max_anisotropy: 0.0,
    border_color: vk::BorderColor::INT_OPAQUE_BLACK,
    unnormalized_coordinates: vk::FALSE,
    compare_enable: vk::FALSE,
    compare_op: vk::CompareOp::NEVER,
    mipmap_mode: vk::SamplerMipmapMode::NEAREST,
    mip_lod_bias: 0.0,
    max_lod: 0.0,
    min_lod: 0.0,
    _marker: PhantomData,
  };
  unsafe { device.create_sampler(&sampler_create_info, None) }.map_err(|err| err.into())
}

pub struct DescriptorPool {
  pub texture_layout: vk::DescriptorSetLayout,
  texture_sampler: vk::Sampler,

  pool: vk::DescriptorPool,
}

impl DescriptorPool {
  const SET_COUNT: u32 = 3;

  const SIZES: [vk::DescriptorPoolSize; 1] = [vk::DescriptorPoolSize {
    ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
    descriptor_count: 1,
  }];

  fn graphics_layout_bindings<'a>(
    texture_sampler: *const vk::Sampler,
  ) -> [vk::DescriptorSetLayoutBinding<'a>; 1] {
    [vk::DescriptorSetLayoutBinding {
      binding: 0,
      descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::FRAGMENT,
      p_immutable_samplers: texture_sampler,
      _marker: PhantomData,
    }]
  }

  pub fn new(device: &ash::Device) -> Result<Self, OutOfMemoryError> {
    let texture_sampler = create_texture_sampler(device)?;

    let texture_layout = Self::create_graphics_layout(device, texture_sampler)?;

    let pool = {
      let pool_create_info = vk::DescriptorPoolCreateInfo {
        s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
        p_next: ptr::null(),
        pool_size_count: Self::SIZES.len() as u32,
        p_pool_sizes: Self::SIZES.as_ptr(),
        max_sets: Self::SET_COUNT,
        flags: vk::DescriptorPoolCreateFlags::empty(),
        _marker: PhantomData,
      };
      unsafe { device.create_descriptor_pool(&pool_create_info, None) }
    }?;

    Ok(Self {
      texture_sampler,
      texture_layout,
      pool,
    })
  }

  pub fn allocate_sets(
    &mut self,
    device: &ash::Device,
    layouts: &[vk::DescriptorSetLayout],
  ) -> Result<Vec<vk::DescriptorSet>, vk::Result> {
    let allocate_info = vk::DescriptorSetAllocateInfo {
      s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
      p_next: ptr::null(),
      descriptor_pool: self.pool,
      descriptor_set_count: layouts.len() as u32,
      p_set_layouts: layouts.as_ptr(),
      _marker: PhantomData,
    };
    unsafe { device.allocate_descriptor_sets(&allocate_info) }
  }

  fn create_graphics_layout(
    device: &ash::Device,
    texture_sampler: vk::Sampler,
  ) -> Result<vk::DescriptorSetLayout, OutOfMemoryError> {
    let ptr = &texture_sampler;
    let bindings = Self::graphics_layout_bindings(ptr);
    create_layout(device, &bindings)
  }
}

impl DeviceManuallyDestroyed for DescriptorPool {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_descriptor_pool(self.pool, None);

    device.destroy_descriptor_set_layout(self.texture_layout, None);
    device.destroy_sampler(self.texture_sampler, None);
  }
}

fn create_layout(
  device: &ash::Device,
  bindings: &[vk::DescriptorSetLayoutBinding],
) -> Result<vk::DescriptorSetLayout, OutOfMemoryError> {
  let create_info = vk::DescriptorSetLayoutCreateInfo {
    s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::DescriptorSetLayoutCreateFlags::empty(),
    binding_count: bindings.len() as u32,
    p_bindings: bindings.as_ptr(),
    _marker: PhantomData,
  };
  unsafe { device.create_descriptor_set_layout(&create_info, None) }.map_err(|err| err.into())
}
