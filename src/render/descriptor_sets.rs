use std::ptr::{self};

use ash::vk;

use crate::render::{compute_data::ComputeData, FRAMES_IN_FLIGHT};

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

pub struct DescriptorSets {
  pub graphics_layout: vk::DescriptorSetLayout,
  texture_sampler: vk::Sampler,
  pub compute_layout: vk::DescriptorSetLayout,

  pool: vk::DescriptorPool,

  pub texture_set: vk::DescriptorSet,
  pub compute_sets: [vk::DescriptorSet; FRAMES_IN_FLIGHT],
}

impl DescriptorSets {
  const SET_COUNT: u32 = 3;

  const SIZES: [vk::DescriptorPoolSize; 2] = [
    vk::DescriptorPoolSize {
      ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      descriptor_count: 1,
    },
    vk::DescriptorPoolSize {
      ty: vk::DescriptorType::STORAGE_BUFFER,
      descriptor_count: ComputeData::COMPUTE_BUFFER_COUNT,
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

    let graphics_layout = Self::create_graphics_layout(device, texture_sampler);
    let compute_layout = Self::create_compute_layout(device);

    let pool = Self::create_pool(device);

    let layouts = [graphics_layout, compute_layout, compute_layout];
    let allocate_info = vk::DescriptorSetAllocateInfo {
      s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
      p_next: ptr::null(),
      descriptor_pool: pool,
      descriptor_set_count: layouts.len() as u32,
      p_set_layouts: layouts.as_ptr(),
    };
    let descriptor_sets = unsafe {
      device
        .allocate_descriptor_sets(&allocate_info)
        .expect("Failed to allocate descriptor set")
    };

    Self {
      texture_sampler,
      graphics_layout,
      compute_layout,
      pool,
      texture_set: descriptor_sets[0],
      compute_sets: [descriptor_sets[1], descriptor_sets[2]],
    }
  }

  fn create_pool(device: &ash::Device) -> vk::DescriptorPool {
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

  // write texture set and all compute data buffer sets
  pub fn write_sets(
    &mut self,
    device: &ash::Device,
    texture_view: vk::ImageView,
    compute_data: &ComputeData,
  ) {
    let texture_info = vk::DescriptorImageInfo {
      sampler: vk::Sampler::null(), // indicated and set as constant in the layout
      image_view: texture_view,
      image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    };
    let texture_write = vk::WriteDescriptorSet {
      s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
      p_next: ptr::null(),
      dst_set: self.texture_set,
      dst_binding: 0,
      dst_array_element: 0,
      descriptor_count: 1,
      descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      p_buffer_info: ptr::null(),
      p_image_info: &texture_info,
      p_texel_buffer_view: ptr::null(),
    };

    let shader_output_0 = vk::DescriptorBufferInfo {
      buffer: compute_data.output[0].buffer,
      offset: 0,
      range: vk::WHOLE_SIZE,
    };
    let shader_output_1 = vk::DescriptorBufferInfo {
      buffer: compute_data.output[1].buffer,
      offset: 0,
      range: vk::WHOLE_SIZE,
    };
    // write for first set and read for second
    let instance_compute_0 = vk::DescriptorBufferInfo {
      buffer: compute_data.instance_compute[0],
      offset: 0,
      range: vk::WHOLE_SIZE,
    };
    // write for second set and read for first
    let instance_compute_1 = vk::DescriptorBufferInfo {
      buffer: compute_data.instance_compute[1],
      offset: 0,
      range: vk::WHOLE_SIZE,
    };

    let base = vk::WriteDescriptorSet {
      s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
      p_next: ptr::null(),
      dst_set: vk::DescriptorSet::null(),
      dst_binding: 0,
      dst_array_element: 0,
      descriptor_count: 1,
      descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
      p_buffer_info: ptr::null(),
      p_image_info: ptr::null(),
      p_texel_buffer_view: ptr::null(),
    };
    let first_set_writes = [
      vk::WriteDescriptorSet {
        dst_set: self.compute_sets[0],
        p_buffer_info: &shader_output_0,
        dst_binding: 0,
        ..base
      },
      vk::WriteDescriptorSet {
        dst_set: self.compute_sets[0],
        p_buffer_info: &instance_compute_1,
        dst_binding: 1,
        ..base
      },
      vk::WriteDescriptorSet {
        dst_set: self.compute_sets[0],
        p_buffer_info: &instance_compute_0,
        dst_binding: 2,
        ..base
      },
    ];

    let second_set_shader_output_write = vk::WriteDescriptorSet {
      dst_set: self.compute_sets[1],
      p_buffer_info: &shader_output_1,
      dst_binding: 0,
      ..base
    };

    let from_first_to_second_copy_base = vk::CopyDescriptorSet {
      s_type: vk::StructureType::COPY_DESCRIPTOR_SET,
      p_next: ptr::null(),
      src_set: self.compute_sets[0],
      src_binding: 0,
      src_array_element: 0,
      dst_set: self.compute_sets[1],
      dst_binding: 0,
      dst_array_element: 0,
      descriptor_count: 1,
    };
    // second set has instance_compute buffers reverted
    let copies = [
      vk::CopyDescriptorSet {
        src_binding: 1,
        dst_binding: 2,
        ..from_first_to_second_copy_base
      },
      vk::CopyDescriptorSet {
        src_binding: 2,
        dst_binding: 1,
        ..from_first_to_second_copy_base
      },
    ];

    unsafe {
      device.update_descriptor_sets(
        &[
          texture_write,
          first_set_writes[0],
          first_set_writes[1],
          first_set_writes[2],
          second_set_shader_output_write,
        ],
        &copies,
      );
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_descriptor_pool(self.pool, None);

    device.destroy_descriptor_set_layout(self.graphics_layout, None);
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
