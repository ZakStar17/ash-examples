use std::ptr::{self, addr_of};

use ash::vk;

use crate::{
  constant_buffers::ConstantBuffers, device::QueueFamilies, pipeline::GraphicsPipeline,
  IMAGE_HEIGHT, IMAGE_WIDTH, INDEX_COUNT,
};

pub struct GraphicsCommandBufferPool {
  pool: vk::CommandPool,
  triangle: vk::CommandBuffer,
}

impl GraphicsCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Self {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.graphics.index);

    let triangle = super::allocate_primary_command_buffers(device, pool, 1)[0];

    Self { pool, triangle }
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .expect("Failed to reset command pool");
  }

  pub unsafe fn record(
    &mut self,
    device: &ash::Device,
    render_pass: vk::RenderPass,
    framebuffer: vk::Framebuffer,
    pipeline: &GraphicsPipeline,
    buffers: &ConstantBuffers,
  ) {
    let cb = self.triangle;

    let command_buffer_begin_info = vk::CommandBufferBeginInfo {
      s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
      p_next: ptr::null(),
      p_inheritance_info: ptr::null(),
      flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
    };
    device
      .begin_command_buffer(cb, &command_buffer_begin_info)
      .expect("Failed to start recording command buffer");

    let clear_value = vk::ClearValue {
      color: vk::ClearColorValue {
        float32: [0.0, 0.0, 0.0, 1.0],
      },
    };
    let render_pass_begin_info = vk::RenderPassBeginInfo {
      s_type: vk::StructureType::RENDER_PASS_BEGIN_INFO,
      p_next: ptr::null(),
      render_pass,
      framebuffer,
      // whole render pass
      render_area: vk::Rect2D {
        offset: vk::Offset2D { x: 0, y: 0 },
        extent: vk::Extent2D {
          width: IMAGE_WIDTH,
          height: IMAGE_HEIGHT,
        },
      },
      clear_value_count: 1,
      p_clear_values: addr_of!(clear_value),
    };

    device.cmd_begin_render_pass(cb, &render_pass_begin_info, vk::SubpassContents::INLINE);
    {
      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
      device.cmd_bind_vertex_buffers(cb, 0, &[buffers.vertex], &[0]);
      device.cmd_bind_index_buffer(cb, buffers.index, 0, vk::IndexType::UINT16);

      // draw static objects
      device.cmd_draw_indexed(cb, INDEX_COUNT as u32, 1, 0, 0, 0);
    }
    device.cmd_end_render_pass(cb);

    device
      .end_command_buffer(cb)
      .expect("Failed to finish recording command buffer")
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
