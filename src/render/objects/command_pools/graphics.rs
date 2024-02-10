use std::ptr::{self, addr_of};

use ash::vk;

use crate::{
  render::{
    objects::{device::QueueFamilies, ConstantAllocatedObjects, DescriptorSets, GraphicsPipeline},
    render_object::INDICES,
    RenderPosition, BACKGROUND_COLOR,
  },
  utility,
};

pub struct GraphicsCommandBufferPool {
  pool: vk::CommandPool,
  pub triangle: vk::CommandBuffer,
}

impl GraphicsCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Self {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.graphics.index);

    let buffers = super::allocate_primary_command_buffers(device, pool, 1);

    Self {
      pool,
      triangle: buffers[0],
    }
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
    descriptor_sets: &DescriptorSets,
    extent: vk::Extent2D,
    framebuffer: vk::Framebuffer,
    pipeline: &GraphicsPipeline,
    constant_allocated_objects: &ConstantAllocatedObjects,
    position: &RenderPosition, // position of the object to be rendered
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
      p_clear_values: addr_of!(clear_value),
    };

    device.cmd_begin_render_pass(cb, &render_pass_begin_info, vk::SubpassContents::INLINE);
    {
      device.cmd_bind_descriptor_sets(
        cb,
        vk::PipelineBindPoint::GRAPHICS,
        pipeline.layout,
        0,
        &[descriptor_sets.pool.texture],
        &[],
      );
      device.cmd_push_constants(
        cb,
        pipeline.layout,
        vk::ShaderStageFlags::VERTEX,
        0,
        utility::any_as_u8_slice(position),
      );
      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, **pipeline);
      device.cmd_bind_vertex_buffers(cb, 0, &[constant_allocated_objects.vertex], &[0]);
      device.cmd_bind_index_buffer(
        cb,
        constant_allocated_objects.index,
        0,
        vk::IndexType::UINT16,
      );
      device.cmd_draw_indexed(cb, INDICES.len() as u32, 1, 0, 0, 0);
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
