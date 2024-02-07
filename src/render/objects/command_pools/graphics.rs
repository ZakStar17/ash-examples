use std::ptr::{self, addr_of};

use ash::vk;

use crate::{
  render::{
    objects::{constant_buffers::ConstantBuffers, device::QueueFamilies, GraphicsPipeline},
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
    queue_families: &QueueFamilies,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
    framebuffer: vk::Framebuffer,
    pipeline: &GraphicsPipeline,
    buffers: &ConstantBuffers,
    swapchain_image: vk::Image,
    position: &RenderPosition,  // position of the object to be rendered
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
      device.cmd_push_constants(
        cb,
        pipeline.layout,
        vk::ShaderStageFlags::VERTEX,
        0,
        utility::any_as_u8_slice(position),
      );
      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, **pipeline);
      device.cmd_bind_vertex_buffers(cb, 0, &[buffers.vertex], &[0]);
      device.cmd_bind_index_buffer(cb, buffers.index, 0, vk::IndexType::UINT16);
      device.cmd_draw_indexed(cb, INDICES.len() as u32, 1, 0, 0, 0);
    }
    device.cmd_end_render_pass(cb);

    if queue_families.presentation != queue_families.graphics {
      let subresource_range = vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 1,
      };
      let release = vk::ImageMemoryBarrier {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        dst_access_mask: vk::AccessFlags::NONE, // should be NONE for ownership release
        old_layout: vk::ImageLayout::PRESENT_SRC_KHR,
        new_layout: vk::ImageLayout::PRESENT_SRC_KHR,
        src_queue_family_index: queue_families.get_graphics_index(),
        dst_queue_family_index: queue_families.get_presentation_index(),
        image: swapchain_image,
        subresource_range,
      };
      device.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        vk::DependencyFlags::empty(),
        &[],
        &[],
        &[release],
      );
    }

    device
      .end_command_buffer(cb)
      .expect("Failed to finish recording command buffer")
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
