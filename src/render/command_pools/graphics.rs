use std::{marker::PhantomData, ptr};

use ash::vk;

use crate::{
  render::{
    descriptor_sets::DescriptorPool,
    device_destroyable::DeviceManuallyDestroyed,
    errors::OutOfMemoryError,
    gpu_data::GPUData,
    initialization::device::QueueFamilies,
    pipelines::GraphicsPipeline,
    render_object::{RenderPosition, QUAD_INDICES},
  },
  utility, BACKGROUND_COLOR,
};

pub struct GraphicsCommandBufferPool {
  pool: vk::CommandPool,
  pub main: vk::CommandBuffer,
}

impl GraphicsCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Result<Self, vk::Result> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_graphics_index())?;

    let main = super::allocate_primary_command_buffers(device, pool, 1)?[0];

    Ok(Self { pool, main })
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) -> Result<(), OutOfMemoryError> {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .map_err(|err| err.into())
  }

  pub unsafe fn record_main(
    &mut self,
    device: &ash::Device,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
    framebuffer: vk::Framebuffer,
    pipeline: &GraphicsPipeline,
    descriptor_pool: &DescriptorPool,
    data: &GPUData,
    position: &RenderPosition, // Ferris's position
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.main;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    // in this case the render pass takes care of all internal queue synchronization
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
        &[descriptor_pool.texture_set],
        &[],
      );
      device.cmd_push_constants(
        cb,
        pipeline.layout,
        vk::ShaderStageFlags::VERTEX,
        0,
        utility::any_as_u8_slice(position),
      );
      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline.current);
      device.cmd_bind_vertex_buffers(cb, 0, &[data.vertex_buffer], &[0]);
      device.cmd_bind_index_buffer(cb, data.index_buffer, 0, vk::IndexType::UINT16);
      device.cmd_draw_indexed(cb, QUAD_INDICES.len() as u32, 1, 0, 0, 0);

      device.cmd_end_render_pass(cb);
    }

    device.end_command_buffer(cb)?;
    Ok(())
  }
}

impl DeviceManuallyDestroyed for GraphicsCommandBufferPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
