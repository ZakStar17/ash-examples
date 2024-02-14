use std::{
  mem::size_of,
  ptr::{self, addr_of},
};

use ash::vk;

use crate::render::{
  push_constants::SpritePushConstants,
  shaders,
  vertices::{vertex_input_state_create_info, InstanceVertex},
  Vertex,
};

use super::DescriptorSets;

fn create_layout(
  device: &ash::Device,
  descriptor_set_layouts: &[vk::DescriptorSetLayout],
  push_constant_ranges: &[vk::PushConstantRange],
) -> vk::PipelineLayout {
  let layout_create_info = vk::PipelineLayoutCreateInfo {
    s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::PipelineLayoutCreateFlags::empty(),
    set_layout_count: descriptor_set_layouts.len() as u32,
    p_set_layouts: descriptor_set_layouts.as_ptr(),
    push_constant_range_count: push_constant_ranges.len() as u32,
    p_push_constant_ranges: push_constant_ranges.as_ptr(),
  };
  unsafe {
    device
      .create_pipeline_layout(&layout_create_info, None)
      .expect("Failed to create pipeline layout")
  }
}

pub struct Pipelines {
  pub player_layout: vk::PipelineLayout,
  pub player: vk::Pipeline,

  pub projectiles_layout: vk::PipelineLayout,
  pub projectiles: vk::Pipeline,
}

impl Pipelines {
  pub fn new(
    device: &ash::Device,
    cache: vk::PipelineCache,
    render_pass: vk::RenderPass,
    descriptor_sets: &DescriptorSets,
    extent: vk::Extent2D,
  ) -> Self {
    let push_constant_range = vk::PushConstantRange {
      stage_flags: vk::ShaderStageFlags::VERTEX,
      offset: 0,
      size: size_of::<SpritePushConstants>() as u32,
    };
    let player_layout = create_layout(device, &[descriptor_sets.layout], &[push_constant_range]);
    let projectiles_layout = create_layout(device, &[descriptor_sets.layout], &[]);

    let (player, projectiles) = Self::create_pipelines(
      device,
      player_layout,
      projectiles_layout,
      cache,
      render_pass,
      extent,
    );

    Self {
      player_layout,
      player,
      projectiles_layout,
      projectiles,
    }
  }

  fn create_pipelines(
    device: &ash::Device,

    player_layout: vk::PipelineLayout,
    projectiles_layout: vk::PipelineLayout,
    cache: vk::PipelineCache,

    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
  ) -> (vk::Pipeline, vk::Pipeline) {
    let mut player_shader = shaders::player::Shader::load(device);
    let mut projectiles_shader = shaders::projectiles::Shader::load(device);

    let player_shader_stages = player_shader.get_pipeline_shader_creation_info();
    let projectiles_shader_stages = projectiles_shader.get_pipeline_shader_creation_info();

    let player_vertex_input_state = vertex_input_state_create_info!(Vertex);
    let projectiles_vertex_input_state = vertex_input_state_create_info!(Vertex, InstanceVertex);

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
      extent,
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

    let player_create_info = vk::GraphicsPipelineCreateInfo {
      s_type: vk::StructureType::GRAPHICS_PIPELINE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineCreateFlags::ALLOW_DERIVATIVES,
      stage_count: player_shader_stages.len() as u32,
      p_stages: player_shader_stages.as_ptr(),
      p_vertex_input_state: player_vertex_input_state.get(),
      p_input_assembly_state: &input_assembly_state_ci,
      p_tessellation_state: ptr::null(),
      p_viewport_state: &viewport_state,
      p_rasterization_state: &rasterization_state_ci,
      p_multisample_state: &multisample_state_ci,
      p_depth_stencil_state: ptr::null(),
      p_color_blend_state: &color_blend_state,
      p_dynamic_state: ptr::null(),
      layout: player_layout,
      render_pass,
      subpass: 0,
      base_pipeline_handle: vk::Pipeline::null(),
      base_pipeline_index: -1, // -1 for null
    };

    let mut projectiles_create_info = player_create_info.clone();
    projectiles_create_info.flags = vk::PipelineCreateFlags::empty();
    projectiles_create_info.stage_count = projectiles_shader_stages.len() as u32;
    projectiles_create_info.p_stages = projectiles_shader_stages.as_ptr();
    projectiles_create_info.p_vertex_input_state = projectiles_vertex_input_state.get();
    projectiles_create_info.layout = projectiles_layout;
    projectiles_create_info.base_pipeline_index = 0;

    let pipelines = unsafe {
      device
        .create_graphics_pipelines(cache, &[player_create_info, projectiles_create_info], None)
        .expect("Failed to create graphics pipelines")
    };

    unsafe {
      player_shader.destroy_self(device);
      projectiles_shader.destroy_self(device);
    }

    (pipelines[0], pipelines[1])
  }

  pub unsafe fn destroy_self(&mut self, device: &ash::Device) {
    device.destroy_pipeline(self.player, None);
    device.destroy_pipeline_layout(self.player_layout, None);

    device.destroy_pipeline(self.projectiles, None);
    device.destroy_pipeline_layout(self.projectiles_layout, None);
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
    cull_mode: vk::CullModeFlags::FRONT,
    front_face: vk::FrontFace::CLOCKWISE,
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
