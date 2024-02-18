use std::{
  mem::{size_of, size_of_val},
  ptr::{self},
};

use ash::vk;

use crate::render::{compute_data::ComputeData, ComputeOutput, FRAMES_IN_FLIGHT};

use super::constant_allocations::INSTANCE_TEMP;

pub struct DescriptorSets {
  pub graphics_layout: vk::DescriptorSetLayout,
  pub compute_layout: vk::DescriptorSetLayout,

  pool: vk::DescriptorPool,

  pub compute: [vk::DescriptorSet; 2],
  pub texture_set: vk::DescriptorSet,
}

impl DescriptorSets {
  pub fn new(device: &ash::Device, texture_sampler: vk::Sampler) -> Self {
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
      graphics_layout,
      compute_layout,
      pool,
      texture_set: descriptor_sets[0],
      compute: [descriptor_sets[1], descriptor_sets[2]],
    }
  }

  fn create_pool(device: &ash::Device) -> vk::DescriptorPool {
    let sizes = [
      vk::DescriptorPoolSize {
        ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        descriptor_count: 1,
      },
      vk::DescriptorPoolSize {
        ty: vk::DescriptorType::STORAGE_BUFFER,
        descriptor_count: 1 + 2,
      },
    ];
    let pool_create_info = vk::DescriptorPoolCreateInfo {
      s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
      p_next: ptr::null(),
      pool_size_count: sizes.len() as u32,
      p_pool_sizes: sizes.as_ptr(),
      max_sets: 3,
      flags: vk::DescriptorPoolCreateFlags::empty(),
    };

    unsafe {
      device
        .create_descriptor_pool(&pool_create_info, None)
        .expect("Failed to create descriptor pool")
    }
  }

  // texture_set must not be in use
  pub fn write_sets(
    &mut self,
    device: &ash::Device,
    texture_view: vk::ImageView,
    instance: vk::Buffer,
    compute_data: &ComputeData
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

    let instance_info = vk::DescriptorBufferInfo {
      buffer: instance,
      offset: 0,
      range: size_of_val(&INSTANCE_TEMP) as u64,
    };
    let instance_write = vk::WriteDescriptorSet {
      s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
      p_next: ptr::null(),
      dst_set: self.compute[0],
      dst_binding: 0,
      dst_array_element: 0,
      descriptor_count: 1,
      descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
      p_buffer_info: &instance_info,
      p_image_info: ptr::null(),
      p_texel_buffer_view: ptr::null(),
    };
    let mut instance_write_2 = instance_write.clone();
    instance_write_2.dst_set = self.compute[1];

    // todo: clean this
    let compute_output_infos = [
      vk::DescriptorBufferInfo {
        buffer: compute_output[0],
        offset: 0,
        range: size_of::<ComputeOutput>() as u64,
      },
      vk::DescriptorBufferInfo {
        buffer: compute_output[1],
        offset: 0,
        range: size_of::<ComputeOutput>() as u64,
      },
    ];
    let compute_output_write = [
      vk::WriteDescriptorSet {
        s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
        p_next: ptr::null(),
        dst_set: self.compute[0],
        dst_binding: 1,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
        p_buffer_info: &compute_output_infos[0],
        p_image_info: ptr::null(),
        p_texel_buffer_view: ptr::null(),
      },
      vk::WriteDescriptorSet {
        s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
        p_next: ptr::null(),
        dst_set: self.compute[1],
        dst_binding: 1,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
        p_buffer_info: &compute_output_infos[1],
        p_image_info: ptr::null(),
        p_texel_buffer_view: ptr::null(),
      },
    ];

    unsafe {
      device.update_descriptor_sets(
        &[
          texture_write,
          instance_write,
          instance_write_2,
          compute_output_write[0],
          compute_output_write[1],
        ],
        &[],
      );
    }
  }

  fn create_graphics_layout(
    device: &ash::Device,
    texture_sampler: vk::Sampler,
  ) -> vk::DescriptorSetLayout {
    let bindings = [vk::DescriptorSetLayoutBinding {
      binding: 0,
      descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::FRAGMENT,
      p_immutable_samplers: &texture_sampler,
    }];
    create_layout(device, &bindings)
  }

  fn create_compute_layout(device: &ash::Device) -> vk::DescriptorSetLayout {
    let bindings = [
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
    ];
    create_layout(device, &bindings)
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_descriptor_pool(self.pool, None);

    device.destroy_descriptor_set_layout(self.graphics_layout, None);
    device.destroy_descriptor_set_layout(self.compute_layout, None);
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
