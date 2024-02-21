use std::ptr::{self};

use ash::vk;

use crate::{
  render::{compute_data::ComputeData, FRAMES_IN_FLIGHT},
  utility,
};

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
  pub texture1_layout: vk::DescriptorSetLayout,
  texture_sampler: vk::Sampler,
  pub storage1_layout: vk::DescriptorSetLayout,
  pub storage2_layout: vk::DescriptorSetLayout,

  pool: vk::DescriptorPool,

  pub texture_set: vk::DescriptorSet,

  pub compute_output_sets: [vk::DescriptorSet; FRAMES_IN_FLIGHT],
  pub instance_sets: [vk::DescriptorSet; FRAMES_IN_FLIGHT + 1],
}

impl DescriptorSets {
  const SET_COUNT: usize = 6;

  const SIZES: [vk::DescriptorPoolSize; 2] = [
    vk::DescriptorPoolSize {
      ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      descriptor_count: 1, // texture
    },
    vk::DescriptorPoolSize {
      ty: vk::DescriptorType::STORAGE_BUFFER,
      // compute output and instance
      descriptor_count: (FRAMES_IN_FLIGHT + (FRAMES_IN_FLIGHT + 1)) as u32,
    },
  ];

  pub fn new(device: &ash::Device) -> Self {
    let texture_sampler = create_texture_sampler(device);

    let texture1_layout = Self::texture1_layout(device, texture_sampler);
    let storage1_layout = Self::storage1_layout(device);
    let storage2_layout = Self::storage2_layout(device);

    let pool = Self::create_pool(device);

    let layouts = utility::conc_arrays!(
      Self::SET_COUNT,
      [texture1_layout],
      [storage1_layout; FRAMES_IN_FLIGHT],
      [storage2_layout; FRAMES_IN_FLIGHT + 1]
    );
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
    let mut desc_iter = descriptor_sets.into_iter();

    Self {
      texture_sampler,
      texture1_layout,
      storage1_layout,
      storage2_layout,
      pool,
      texture_set: desc_iter.next().unwrap(),
      compute_output_sets: utility::repeat_in_array!(desc_iter.next().unwrap(), FRAMES_IN_FLIGHT),
      instance_sets: utility::repeat_in_array!(desc_iter.next().unwrap(), FRAMES_IN_FLIGHT + 1),
    }
  }

  fn create_pool(device: &ash::Device) -> vk::DescriptorPool {
    let pool_create_info = vk::DescriptorPoolCreateInfo {
      s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
      p_next: ptr::null(),
      pool_size_count: Self::SIZES.len() as u32,
      p_pool_sizes: Self::SIZES.as_ptr(),
      max_sets: Self::SET_COUNT as u32,
      flags: vk::DescriptorPoolCreateFlags::empty(),
    };
    unsafe {
      device
        .create_descriptor_pool(&pool_create_info, None)
        .expect("Failed to create descriptor pool")
    }
  }

  fn texture1_layout(
    device: &ash::Device,
    texture_sampler: vk::Sampler,
  ) -> vk::DescriptorSetLayout {
    let binding = vk::DescriptorSetLayoutBinding {
      binding: 0,
      descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::FRAGMENT,
      p_immutable_samplers: &texture_sampler,
    };
    create_layout(device, &[binding])
  }

  fn storage1_layout(device: &ash::Device) -> vk::DescriptorSetLayout {
    let storage = vk::DescriptorSetLayoutBinding {
      binding: 0,
      descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::COMPUTE,
      p_immutable_samplers: ptr::null(),
    };
    create_layout(device, &[storage])
  }

  fn storage2_layout(device: &ash::Device) -> vk::DescriptorSetLayout {
    let storage0 = vk::DescriptorSetLayoutBinding {
      binding: 0,
      descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::COMPUTE,
      p_immutable_samplers: ptr::null(),
    };
    let mut storage1 = storage0.clone();
    storage1.binding = 1;
    create_layout(device, &[storage0, storage1])
  }

  // write texture set and all compute data buffer sets
  pub fn write_sets(
    &mut self,
    device: &ash::Device,
    texture_view: vk::ImageView,
    compute_data: &ComputeData,
  ) {
    let base_write = vk::WriteDescriptorSet {
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
    let base_0_to_1_copy = vk::CopyDescriptorSet {
      s_type: vk::StructureType::COPY_DESCRIPTOR_SET,
      p_next: ptr::null(),
      src_set: vk::DescriptorSet::null(),
      src_binding: 0,
      src_array_element: 0,
      dst_set: vk::DescriptorSet::null(),
      dst_binding: 1,
      dst_array_element: 0,
      descriptor_count: 1,
    };

    let texture_info = vk::DescriptorImageInfo {
      sampler: vk::Sampler::null(), // indicated and set as constant in the layout
      image_view: texture_view,
      image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    };
    let texture_write = vk::WriteDescriptorSet {
      dst_set: self.texture_set,
      descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      p_image_info: &texture_info,
      ..base_write
    };

    let output_0 = vk::DescriptorBufferInfo {
      buffer: compute_data.output[0].buffer,
      offset: 0,
      range: vk::WHOLE_SIZE,
    };
    let output_1 = vk::DescriptorBufferInfo {
      buffer: compute_data.output[1].buffer,
      offset: 0,
      range: vk::WHOLE_SIZE,
    };
    let output_writes = [
      vk::WriteDescriptorSet {
        dst_set: self.compute_output_sets[0],
        p_buffer_info: &output_0,
        ..base_write
      },
      vk::WriteDescriptorSet {
        dst_set: self.compute_output_sets[1],
        p_buffer_info: &output_1,
        ..base_write
      },
    ];

    // 0 -> 1 -> 2 -> 0 (-> = "writes to")
    let instance_0 = vk::DescriptorBufferInfo {
      buffer: compute_data.instance_compute[0],
      offset: 0,
      range: vk::WHOLE_SIZE,
    };
    let instance_1 = vk::DescriptorBufferInfo {
      buffer: compute_data.instance_compute[1],
      offset: 0,
      range: vk::WHOLE_SIZE,
    };
    let instance_2 = vk::DescriptorBufferInfo {
      buffer: compute_data.instance_compute[2],
      offset: 0,
      range: vk::WHOLE_SIZE,
    };
    let binding0_instance_writes = [
      vk::WriteDescriptorSet {
        dst_set: self.instance_sets[0],
        p_buffer_info: &instance_0,
        ..base_write
      },
      vk::WriteDescriptorSet {
        dst_set: self.instance_sets[1],
        p_buffer_info: &instance_1,
        ..base_write
      },
      vk::WriteDescriptorSet {
        dst_set: self.instance_sets[2],
        p_buffer_info: &instance_2,
        ..base_write
      },
    ];
    let binding1_instance_copies = [
      vk::CopyDescriptorSet {
        src_set: self.instance_sets[1],
        dst_set: self.instance_sets[0],
        ..base_0_to_1_copy
      },
      vk::CopyDescriptorSet {
        src_set: self.instance_sets[2],
        dst_set: self.instance_sets[1],
        ..base_0_to_1_copy
      },
      vk::CopyDescriptorSet {
        src_set: self.instance_sets[0],
        dst_set: self.instance_sets[2],
        ..base_0_to_1_copy
      },
    ];

    let writes = utility::conc_arrays!(
      Self::SET_COUNT,
      [texture_write],
      output_writes,
      binding0_instance_writes
    );

    println!(
      "writes {:#?}, \ncopies {:#?}",
      binding0_instance_writes, binding1_instance_copies
    );
    unsafe {
      device.update_descriptor_sets(&writes, &binding1_instance_copies);
    }
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_descriptor_pool(self.pool, None);

    device.destroy_descriptor_set_layout(self.texture1_layout, None);
    device.destroy_descriptor_set_layout(self.storage1_layout, None);
    device.destroy_descriptor_set_layout(self.storage2_layout, None);
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
