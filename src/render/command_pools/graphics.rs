use std::{marker::PhantomData, ptr};

use ash::vk;

use crate::{
  render::{
    descriptor_sets::DescriptorPool,
    device_destroyable::DeviceManuallyDestroyed,
    errors::OutOfMemoryError,
    gpu_data::FerrisModelData,
    initialization::device::QueueFamilies,
    pipelines::GraphicsPipeline,
    render_object::{RenderPosition, INDICES},
  },
  utility, BACKGROUND_COLOR,
};

pub struct GraphicsCommandBufferPool {
  pool: vk::CommandPool,
  pub triangle: vk::CommandBuffer,
}

impl GraphicsCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Result<Self, vk::Result> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_graphics_index())?;

    let triangle = super::allocate_primary_command_buffers(device, pool, 1)?[0];

    Ok(Self { pool, triangle })
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) -> Result<(), vk::Result> {
    device.reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
  }

  pub unsafe fn record_triangle(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    render_pass: vk::RenderPass,
    descriptors: &DescriptorPool,
    extent: vk::Extent2D,
    framebuffer: vk::Framebuffer,
    pipeline: &GraphicsPipeline,
    // contains index and vertex buffer
    ferris_model: &FerrisModelData,
    position: &RenderPosition, // Ferris's position
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.triangle;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    {
      let clear_value = vk::ClearValue {
        color: BACKGROUND_COLOR,
      };
      let render_pass_begin_info = vk::RenderPassBeginInfo {
        s_type: vk::StructureType::RENDER_PASS_BEGIN_INFO,
        p_next: ptr::null(),
        render_pass,
        framebuffer,
        // whole image
        render_area: vk::Rect2D {
          offset: vk::Offset2D { x: 0, y: 0 },
          extent,
        },
        clear_value_count: 1,
        p_clear_values: &clear_value,
        _marker: PhantomData,
      };
      device.cmd_begin_render_pass(cb, &render_pass_begin_info, vk::SubpassContents::INLINE);

      device.cmd_bind_descriptor_sets(
        cb,
        vk::PipelineBindPoint::GRAPHICS,
        pipeline.layout,
        0,
        &[descriptors.texture],
        &[],
      );
      device.cmd_push_constants(
        cb,
        pipeline.layout,
        vk::ShaderStageFlags::VERTEX,
        0,
        utility::any_as_u8_slice(position),
      );
      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
      device.cmd_bind_vertex_buffers(cb, 0, &[ferris_model.vertex], &[0]);
      device.cmd_bind_index_buffer(cb, ferris_model.index, 0, vk::IndexType::UINT16);
      device.cmd_draw_indexed(cb, INDICES.len() as u32, 1, 0, 0, 0);

      device.cmd_end_render_pass(cb);
    }

    // image has 1 mip_level / 1 array layer
    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    device.end_command_buffer(self.triangle)?;

    Ok(())
  }
}

impl DeviceManuallyDestroyed for GraphicsCommandBufferPool {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
