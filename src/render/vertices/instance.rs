use std::mem::{offset_of, size_of};

use ash::vk;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct InstanceVertex {
  pub pos: [f32; 2],
}

impl InstanceVertex {
  const ATTRIBUTE_SIZE: usize = 1;

  pub const fn get_binding_description(binding: u32) -> vk::VertexInputBindingDescription {
    vk::VertexInputBindingDescription {
      binding,
      stride: size_of::<Self>() as u32,
      input_rate: vk::VertexInputRate::INSTANCE,
    }
  }

  pub const fn get_attribute_descriptions(
    offset: u32,
    binding: u32,
  ) -> [vk::VertexInputAttributeDescription; Self::ATTRIBUTE_SIZE] {
    [vk::VertexInputAttributeDescription {
      location: offset,
      binding,
      format: vk::Format::R32G32_SFLOAT,
      offset: offset_of!(Self, pos) as u32,
    }]
  }
}
