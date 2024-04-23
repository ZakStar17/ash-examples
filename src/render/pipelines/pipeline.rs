use std::{
  mem::size_of,
  ops::Deref,
  pin::pin,
  ptr::{self, addr_of},
};

use ash::vk;

use crate::render::{
  shaders::Shader,
  vertex::{PipelineVertexInputStateCreateInfoGen, Vertex},
  RenderPosition,
};

use super::DescriptorSets;

pub struct GraphicsPipeline {
  pub layout: vk::PipelineLayout,
  vk_obj: vk::Pipeline,
  old: Option<vk::Pipeline>,
}

impl Deref for GraphicsPipeline {
  type Target = vk::Pipeline;

  fn deref(&self) -> &Self::Target {
    &self.vk_obj
  }
}

impl GraphicsPipeline {
  pub fn create(
    device: &ash::Device,
    cache: vk::PipelineCache,
    render_pass: vk::RenderPass,
    descriptor_sets: &DescriptorSets,
    extent: vk::Extent2D,
  ) -> Self {
    let push_constant_range = vk::PushConstantRange {
      stage_flags: vk::ShaderStageFlags::VERTEX,
      offset: 0,
      size: size_of::<RenderPosition>() as u32,
    };
    let layout_create_info = vk::PipelineLayoutCreateInfo {
      s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineLayoutCreateFlags::empty(),
      set_layout_count: 1,
      p_set_layouts: &descriptor_sets.layout,
      push_constant_range_count: 1,
      p_push_constant_ranges: &push_constant_range,
    };
    let layout = unsafe {
      device
        .create_pipeline_layout(&layout_create_info, None)
        .expect("Failed to create pipeline layout")
    };

    let pipeline = Self::create_with_base(
      device,
      layout,
      cache,
      vk::Pipeline::null(),
      render_pass,
      extent,
    );

    Self {
      layout,
      vk_obj: pipeline,
      old: None,
    }
  }

  pub fn recreate(
    &mut self,
    device: &ash::Device,
    cache: vk::PipelineCache,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
  ) {
    assert!(self.old.is_none());

    let mut new =
      Self::create_with_base(device, self.layout, cache, self.vk_obj, render_pass, extent);

    let old = {
      std::mem::swap(&mut self.vk_obj, &mut new);
      new
    };

    self.old = Some(old);
  }

  // destroy old pipeline once it stops being used
  pub unsafe fn destroy_old(&mut self, device: &ash::Device) {
    if let Some(old) = self.old {
      device.destroy_pipeline(old, None);
      self.old = None;
    }
  }

  fn create_with_base(
    device: &ash::Device,
    layout: vk::PipelineLayout,
    cache: vk::PipelineCache,
    base: vk::Pipeline,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
  ) -> vk::Pipeline {
    let mut shader = Shader::load(device);
    let shader_stages = shader.get_pipeline_shader_creation_info();

    let vertex_input_state_gen = pin!(Vertex::get_input_state_create_info_gen(0, 0));
    let vertex_input_state =
      PipelineVertexInputStateCreateInfoGen::gen(vertex_input_state_gen.as_ref());

    let input_assembly_state_ci = triangle_input_assembly_state();

    // full image viewport and scissor
    let viewport = vk::Viewport {
      x: 0.0,
      y: 0.0,
      width: extent.width as f32,
      height: extent.height as f32,
      min_depth: 0.0,
      max_depth: 1.0,
    };
    let scissor = vk::Rect2D {
      offset: vk::Offset2D { x: 0, y: 0 },
      extent: vk::Extent2D {
        width: extent.width,
        height: extent.height,
      },
    };
    let viewport_state = vk::PipelineViewportStateCreateInfo {
      s_type: vk::StructureType::PIPELINE_VIEWPORT_STATE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineViewportStateCreateFlags::empty(),
      scissor_count: 1,
      p_scissors: addr_of!(scissor),
      viewport_count: 1,
      p_viewports: addr_of!(viewport),
    };

    let rasterization_state_ci = no_depth_rasterization_state();
    let multisample_state_ci = no_multisample_state();

    let attachment_state = vk::PipelineColorBlendAttachmentState {
      // blend by opacity
      blend_enable: vk::TRUE,
      color_write_mask: vk::ColorComponentFlags::RGBA,

      // final_color = (src_alpha * src_color) + ((1 - src_alpha) * dst_color)
      src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
      dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
      color_blend_op: vk::BlendOp::ADD,

      // final_alpha = src_alpha
      src_alpha_blend_factor: vk::BlendFactor::ONE,
      dst_alpha_blend_factor: vk::BlendFactor::ZERO,
      alpha_blend_op: vk::BlendOp::ADD,
    };
    let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
      s_type: vk::StructureType::PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineColorBlendStateCreateFlags::empty(),
      logic_op_enable: vk::FALSE,
      logic_op: vk::LogicOp::COPY, // disabled
      attachment_count: 1,
      p_attachments: addr_of!(attachment_state),
      blend_constants: [0.0, 0.0, 0.0, 0.0],
    };

    let create_info = vk::GraphicsPipelineCreateInfo {
      s_type: vk::StructureType::GRAPHICS_PIPELINE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineCreateFlags::empty(),
      stage_count: shader_stages.len() as u32,
      p_stages: shader_stages.as_ptr(),
      p_vertex_input_state: vertex_input_state.as_ptr(),
      p_input_assembly_state: &input_assembly_state_ci,
      p_tessellation_state: ptr::null(),
      p_viewport_state: &viewport_state,
      p_rasterization_state: &rasterization_state_ci,
      p_multisample_state: &multisample_state_ci,
      p_depth_stencil_state: ptr::null(),
      p_color_blend_state: &color_blend_state,
      p_dynamic_state: ptr::null(),
      layout,
      render_pass,
      subpass: 0,
      base_pipeline_handle: base,
      base_pipeline_index: -1, // -1 for null
    };
    let pipeline = unsafe {
      device
        .create_graphics_pipelines(cache, &[create_info], None)
        .expect("Failed to create graphics pipelines")[0]
    };

    unsafe {
      shader.destroy_self(device);
    }

    pipeline
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    self.destroy_old(device);
    device.destroy_pipeline(self.vk_obj, None);
    device.destroy_pipeline_layout(self.layout, None);
  }
}

fn triangle_input_assembly_state() -> vk::PipelineInputAssemblyStateCreateInfo {
  vk::PipelineInputAssemblyStateCreateInfo {
    s_type: vk::StructureType::PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
    flags: vk::PipelineInputAssemblyStateCreateFlags::empty(),
    p_next: ptr::null(),
    // defines that there exists a special value that restarts the assembly
    primitive_restart_enable: vk::FALSE,
    topology: vk::PrimitiveTopology::TRIANGLE_LIST,
  }
}

// rasterization with no depth and no culling
fn no_depth_rasterization_state() -> vk::PipelineRasterizationStateCreateInfo {
  vk::PipelineRasterizationStateCreateInfo {
    s_type: vk::StructureType::PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::PipelineRasterizationStateCreateFlags::empty(),
    depth_clamp_enable: vk::FALSE,
    cull_mode: vk::CullModeFlags::NONE,
    front_face: vk::FrontFace::CLOCKWISE, // doesn't matter if cull_mode is none
    line_width: 1.0,
    polygon_mode: vk::PolygonMode::FILL,
    rasterizer_discard_enable: vk::FALSE,
    depth_bias_clamp: 0.0,
    depth_bias_constant_factor: 0.0,
    depth_bias_enable: vk::FALSE,
    depth_bias_slope_factor: 0.0,
  }
}

fn no_multisample_state() -> vk::PipelineMultisampleStateCreateInfo {
  // everything off
  vk::PipelineMultisampleStateCreateInfo {
    s_type: vk::StructureType::PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
    flags: vk::PipelineMultisampleStateCreateFlags::empty(),
    p_next: ptr::null(),
    rasterization_samples: vk::SampleCountFlags::TYPE_1,
    sample_shading_enable: vk::FALSE,
    min_sample_shading: 0.0,
    p_sample_mask: ptr::null(),
    alpha_to_one_enable: vk::FALSE,
    alpha_to_coverage_enable: vk::FALSE,
  }
}
