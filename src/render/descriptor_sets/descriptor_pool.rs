use std::ptr;

use ash::vk;

use crate::render::compute_data::ComputeData;

fn create_texture_sampler(device: &ash::Device) -> vk::Sampler {
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

pub struct DescriptorPool {
  pub texture_layout: vk::DescriptorSetLayout,
  texture_sampler: vk::Sampler,

  pub compute_layout: vk::DescriptorSetLayout,

  pool: vk::DescriptorPool,
}

impl DescriptorPool {
  const SET_COUNT: u32 = 3;

  const SIZES: [vk::DescriptorPoolSize; 2] = [
    vk::DescriptorPoolSize {
      ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      descriptor_count: 1,
    },
    vk::DescriptorPoolSize {
      ty: vk::DescriptorType::STORAGE_BUFFER,
      descriptor_count: ComputeData::STORAGE_BUFFERS_IN_SETS,
    },
  ];

  fn graphics_layout_bindings(
    texture_sampler: *const vk::Sampler,
  ) -> [vk::DescriptorSetLayoutBinding; 1] {
    [vk::DescriptorSetLayoutBinding {
      binding: 0,
      descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::FRAGMENT,
      p_immutable_samplers: texture_sampler,
    }]
  }

  const COMPUTE_LAYOUT_BINDINGS: [vk::DescriptorSetLayoutBinding; 3] = [
    vk::DescriptorSetLayoutBinding {
      binding: 0,
      descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::COMPUTE,
      p_immutable_samplers: ptr::null(),
    },
    vk::DescriptorSetLayoutBinding {
      binding: 1,
      descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::COMPUTE,
      p_immutable_samplers: ptr::null(),
    },
    vk::DescriptorSetLayoutBinding {
      binding: 2,
      descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::COMPUTE,
      p_immutable_samplers: ptr::null(),
    },
  ];

  pub fn new(device: &ash::Device) -> Self {
    let texture_sampler = create_texture_sampler(device);

    let texture_layout = Self::create_graphics_layout(device, texture_sampler);
    let compute_layout = Self::create_compute_layout(device);

    let pool = {
      let pool_create_info = vk::DescriptorPoolCreateInfo {
        s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
        p_next: ptr::null(),
        pool_size_count: Self::SIZES.len() as u32,
        p_pool_sizes: Self::SIZES.as_ptr(),
        max_sets: Self::SET_COUNT,
        flags: vk::DescriptorPoolCreateFlags::empty(),
      };
      unsafe {
        device
          .create_descriptor_pool(&pool_create_info, None)
          .expect("Failed to create descriptor pool")
      }
    };

    Self {
      texture_sampler,
      texture_layout,
      compute_layout,
      pool,
    }
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
    };
    unsafe { device.allocate_descriptor_sets(&allocate_info) }
  }

  fn create_graphics_layout(
    device: &ash::Device,
    texture_sampler: vk::Sampler,
  ) -> vk::DescriptorSetLayout {
    let ptr = &texture_sampler;
    let bindings = Self::graphics_layout_bindings(ptr);
    create_layout(device, &bindings)
  }

  fn create_compute_layout(device: &ash::Device) -> vk::DescriptorSetLayout {
    create_layout(device, &Self::COMPUTE_LAYOUT_BINDINGS)
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_descriptor_pool(self.pool, None);

    device.destroy_descriptor_set_layout(self.texture_layout, None);
    device.destroy_descriptor_set_layout(self.compute_layout, None);
    device.destroy_sampler(self.texture_sampler, None);
  }
}

fn create_layout(
  device: &ash::Device,
  bindings: &[vk::DescriptorSetLayoutBinding],
) -> vk::DescriptorSetLayout {
  let create_info = vk::DescriptorSetLayoutCreateInfo {
    s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::DescriptorSetLayoutCreateFlags::empty(),
    binding_count: bindings.len() as u32,
    p_bindings: bindings.as_ptr(),
  };
  unsafe {
    device
      .create_descriptor_set_layout(&create_info, None)
      .expect("Failed to create a descriptor set layout")
  }
}
