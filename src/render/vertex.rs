use std::{
  marker::PhantomData,
  mem::size_of,
  pin::Pin,
  ptr::{self, addr_of},
};

use ash::vk;
use memoffset::offset_of;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Vertex {
  pub pos: [f32; 2],
  pub tex_coords: [f32; 2],
}

impl Vertex {
  const ATTRIBUTE_SIZE: usize = 2;

  const fn get_binding_description(binding: u32) -> vk::VertexInputBindingDescription {
    vk::VertexInputBindingDescription {
      binding,
      stride: size_of::<Self>() as u32,
      input_rate: vk::VertexInputRate::VERTEX,
    }
  }

  const fn get_attribute_descriptions(
    offset: u32,
    binding: u32,
  ) -> [vk::VertexInputAttributeDescription; Self::ATTRIBUTE_SIZE] {
    [
      vk::VertexInputAttributeDescription {
        location: offset,
        binding,
        format: vk::Format::R32G32_SFLOAT,
        offset: offset_of!(Self, pos) as u32,
      },
      vk::VertexInputAttributeDescription {
        location: offset + 1,
        binding,
        format: vk::Format::R32G32_SFLOAT,
        offset: offset_of!(Self, tex_coords) as u32,
      },
    ]
  }

  pub fn get_input_state_create_info_gen(
    binding: u32,
    attribute_offset: u32,
  ) -> PipelineVertexInputStateCreateInfoGen {
    PipelineVertexInputStateCreateInfoGen {
      binding_description: Self::get_binding_description(binding),
      attribute_descriptions: Self::get_attribute_descriptions(attribute_offset, binding),
    }
  }
}

pub struct PipelineVertexInputStateCreateInfoGen {
  binding_description: vk::VertexInputBindingDescription,
  attribute_descriptions: [vk::VertexInputAttributeDescription; Vertex::ATTRIBUTE_SIZE],
}

// this struct should not outlive the generator, so it has a phantom field marking its lifetime
pub struct PipelineVertexInputStateCreateInfo<'a> {
  vk_obj: vk::PipelineVertexInputStateCreateInfo,
  phantom: PhantomData<Pin<&'a PipelineVertexInputStateCreateInfoGen>>,
}

impl<'a> PipelineVertexInputStateCreateInfo<'a> {
  fn new(vk_obj: vk::PipelineVertexInputStateCreateInfo) -> Self {
    Self {
      vk_obj,
      phantom: PhantomData,
    }
  }

  pub fn as_ptr(&self) -> *const vk::PipelineVertexInputStateCreateInfo {
    &self.vk_obj
  }
}

impl PipelineVertexInputStateCreateInfoGen {
  pub fn gen<'a>(pinned_self: Pin<&'a Self>) -> PipelineVertexInputStateCreateInfo<'a> {
    PipelineVertexInputStateCreateInfo::new(vk::PipelineVertexInputStateCreateInfo {
      s_type: vk::StructureType::PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineVertexInputStateCreateFlags::empty(),
      vertex_attribute_description_count: pinned_self.attribute_descriptions.len() as u32,
      p_vertex_attribute_descriptions: pinned_self.attribute_descriptions.as_ptr(),
      vertex_binding_description_count: 1,
      p_vertex_binding_descriptions: addr_of!(pinned_self.binding_description),
    })
  }
}
