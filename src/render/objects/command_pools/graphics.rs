use std::ptr::{self, addr_of};

use ash::vk;

use crate::{
  render::{
    objects::{device::QueueFamilies, ConstantAllocatedObjects, DescriptorSets, Pipelines},
    push_constants::SpritePushConstants,
    sprites::SQUARE_INDICES,
    BACKGROUND_COLOR, OUT_OF_BOUNDS_AREA_COLOR,
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
    render_image: vk::Image,
    framebuffer: vk::Framebuffer,
    render_extent: vk::Extent2D,

    swapchain_image: vk::Image,
    swapchain_extent: vk::Extent2D,

    descriptor_sets: &DescriptorSets,
    pipelines: &Pipelines,
    constant_allocated_objects: &ConstantAllocatedObjects,
    player: &SpritePushConstants, // position of the object to be rendered
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

    // 1 mip_level / 1 array layer
    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

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
        extent: render_extent,
      },
      clear_value_count: 1,
      p_clear_values: addr_of!(clear_value),
    };

    device.cmd_begin_render_pass(cb, &render_pass_begin_info, vk::SubpassContents::INLINE);
    {
      device.cmd_bind_descriptor_sets(
        cb,
        vk::PipelineBindPoint::GRAPHICS,
        pipelines.projectiles_layout,
        0,
        &[descriptor_sets.pool.texture],
        &[],
      );
      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipelines.projectiles);
      device.cmd_bind_vertex_buffers(cb, 0, &[constant_allocated_objects.vertex, constant_allocated_objects.instance], &[0, 0]);
      device.cmd_bind_index_buffer(
        cb,
        constant_allocated_objects.index,
        0,
        vk::IndexType::UINT16,
      );
      device.cmd_draw_indexed(cb, SQUARE_INDICES.len() as u32, 2, 0, 4, 0);

      device.cmd_bind_descriptor_sets(
        cb,
        vk::PipelineBindPoint::GRAPHICS,
        pipelines.player_layout,
        0,
        &[descriptor_sets.pool.texture],
        &[],
      );
      device.cmd_push_constants(
        cb,
        pipelines.player_layout,
        vk::ShaderStageFlags::VERTEX,
        0,
        utility::any_as_u8_slice(player),
      );
      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipelines.player);
      device.cmd_draw_indexed(cb, SQUARE_INDICES.len() as u32, 1, 0, 0, 0);
    }
    device.cmd_end_render_pass(cb);

    // change swapchain image layout to transfer dst
    {
      let swapchain_transfer_dst_layout = vk::ImageMemoryBarrier {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags::NONE,
        dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
        old_layout: vk::ImageLayout::UNDEFINED,
        new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        image: swapchain_image,
        subresource_range,
      };
      device.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::TRANSFER,
        vk::DependencyFlags::empty(),
        &[],
        &[],
        &[swapchain_transfer_dst_layout],
      );
    }

    device.cmd_clear_color_image(
      cb,
      swapchain_image,
      vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      &OUT_OF_BOUNDS_AREA_COLOR,
      &[subresource_range],
    );

    {
      // transfer barrier
      let barrier = vk::MemoryBarrier {
        s_type: vk::StructureType::MEMORY_BARRIER,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
      };
      device.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::TRANSFER,
        vk::DependencyFlags::empty(),
        &[barrier],
        &[],
        &[],
      );
    }

    {
      let layers = vk::ImageSubresourceLayers {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        mip_level: 0,
        base_array_layer: 0,
        layer_count: 1,
      };

      let src_width = render_extent.width as i32;
      let src_height = render_extent.height as i32;
      let dst_width = swapchain_extent.width as i32;
      let dst_height = swapchain_extent.height as i32;

      // calculate blit region in swapchain image
      let width_diff = dst_width - src_width;
      let height_diff = dst_height - src_height;
      let dst_start;
      let dst_end;
      if width_diff > height_diff {
        // clamp to height
        let ratio = dst_height as f32 / src_height as f32;
        let resized_width = (src_width as f32 * ratio) as i32;

        let half = (dst_width - resized_width) / 2;
        dst_start = [half, 0];
        dst_end = [half + resized_width, dst_height];
      } else if width_diff == height_diff {
        dst_start = [0, 0];
        dst_end = [dst_width, dst_height];
      } else {
        // clamp to width
        let ratio = dst_width as f32 / src_width as f32;
        let resized_height = (src_height as f32 * ratio) as i32;

        let half = (dst_height - resized_height) / 2;
        dst_start = [0, half];
        dst_end = [dst_width, half + resized_height];
      }
      let blit_region = vk::ImageBlit {
        src_subresource: layers,
        src_offsets: [
          vk::Offset3D { x: 0, y: 0, z: 0 },
          vk::Offset3D {
            x: render_extent.width as i32,
            y: render_extent.height as i32,
            z: 1,
          },
        ],
        dst_subresource: layers,
        dst_offsets: [
          vk::Offset3D {
            x: dst_start[0],
            y: dst_start[1],
            z: 0,
          },
          vk::Offset3D {
            x: dst_end[0],
            y: dst_end[1],
            z: 1,
          },
        ],
      };

      device.cmd_blit_image(
        cb,
        render_image,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        swapchain_image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        &[blit_region],
        vk::Filter::NEAREST,
      );
    }

    // change swapchain image layout to presentation
    {
      let swapchain_presentation_layout = vk::ImageMemoryBarrier {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags::NONE,
        old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        new_layout: vk::ImageLayout::PRESENT_SRC_KHR,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        image: swapchain_image,
        subresource_range,
      };
      device.cmd_pipeline_barrier(
        cb,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::TRANSFER,
        vk::DependencyFlags::empty(),
        &[],
        &[],
        &[swapchain_presentation_layout],
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
