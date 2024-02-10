use std::ptr::{self, addr_of};

use ash::vk;

pub struct DescriptorSets {
  pub layout: vk::DescriptorSetLayout,
  pub pool: DescriptorSetPool,
}

impl DescriptorSets {
  pub fn new(device: &ash::Device) -> Self {
    let layout = create_layout(device);

    let pool = DescriptorSetPool::new(device, layout);
    Self { layout, pool }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    self.pool.destroy_self(device);
    device.destroy_descriptor_set_layout(self.layout, None);
  }
}

fn create_layout(device: &ash::Device) -> vk::DescriptorSetLayout {
  let bindings = [vk::DescriptorSetLayoutBinding {
    binding: 0,
    descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
    descriptor_count: 1,
    stage_flags: vk::ShaderStageFlags::FRAGMENT,
    p_immutable_samplers: ptr::null(),
  }];

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

pub struct DescriptorSetPool {
  pool: vk::DescriptorPool,
  pub texture: vk::DescriptorSet,
}

impl DescriptorSetPool {
  pub fn new(device: &ash::Device, layout: vk::DescriptorSetLayout) -> Self {
    let sizes = [vk::DescriptorPoolSize {
      ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      descriptor_count: 1,
    }];
    let pool_create_info = vk::DescriptorPoolCreateInfo {
      s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
      p_next: ptr::null(),
      pool_size_count: sizes.len() as u32,
      p_pool_sizes: sizes.as_ptr(),
      max_sets: 1,
      flags: vk::DescriptorPoolCreateFlags::empty(),
    };
    let pool = unsafe {
      device
        .create_descriptor_pool(&pool_create_info, None)
        .expect("Failed to create descriptor pool")
    };

    let allocate_info = vk::DescriptorSetAllocateInfo {
      s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
      p_next: ptr::null(),
      descriptor_pool: pool,
      descriptor_set_count: 1,
      p_set_layouts: addr_of!(layout),
    };
    let descriptor_set = unsafe {
      device
        .allocate_descriptor_sets(&allocate_info)
        .expect("Failed to allocate descriptor set")[0]
    };

    Self {
      pool,
      texture: descriptor_set,
    }
  }

  pub fn write_texture(
    &mut self,
    device: &ash::Device,
    texture_view: vk::ImageView,
    sampler: vk::Sampler,
  ) {
    let image_info = vk::DescriptorImageInfo {
      sampler,
      image_view: texture_view,
      image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    };

    let write = vk::WriteDescriptorSet {
      s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
      p_next: ptr::null(),
      dst_set: self.texture,
      dst_binding: 0,
      dst_array_element: 0,
      descriptor_count: 1,
      descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      p_buffer_info: ptr::null(),
      p_image_info: addr_of!(image_info),
      p_texel_buffer_view: ptr::null(),
    };

    unsafe {
      device.update_descriptor_sets(&[write], &[]);
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_descriptor_pool(self.pool, None);
  }
}
