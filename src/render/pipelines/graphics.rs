use std::{
  marker::PhantomData,
  mem::{self, size_of},
  ops::BitOr,
  ptr::{self},
};

use ash::vk::{self, Handle};

use crate::{
  render::{
    data::compute::Bullet, descriptor_sets::DescriptorPool,
    device_destroyable::DeviceManuallyDestroyed, errors::OutOfMemoryError,
    push_constants::SpritePushConstants, shaders, vertices::Vertex,
  },
  vertex_input_state_create_info,
};

use super::PipelineCreationError;

#[derive(Debug, Clone, Copy)]
pub struct GraphicsPipelinesList {
  pub player: vk::Pipeline,
  pub bullets: vk::Pipeline,
}

pub struct GraphicsPipelines {
  // compatible with both player and bullets
  pub layout: vk::PipelineLayout,
  pub current: GraphicsPipelinesList,

  player_shader: shaders::player::Shader,
  bullets_shader: shaders::bullets::Shader,
  old: Option<GraphicsPipelinesList>,
}

impl GraphicsPipelines {
  pub fn new(
    device: &ash::Device,
    cache: vk::PipelineCache,
    render_pass: vk::RenderPass,
    descriptor_pool: &DescriptorPool,
    extent: vk::Extent2D,
  ) -> Result<Self, PipelineCreationError> {
    let layout = Self::create_layout(device, descriptor_pool)?;
    let player_shader =
      shaders::player::Shader::load(device).map_err(PipelineCreationError::ShaderFailed)?;
    let bullets_shader =
      shaders::bullets::Shader::load(device).map_err(PipelineCreationError::ShaderFailed)?;

    let initial = Self::create_with_base(
      device,
      layout,
      cache,
      GraphicsPipelinesList {
        player: vk::Pipeline::null(),
        bullets: vk::Pipeline::null(),
      },
      player_shader,
      bullets_shader,
      render_pass,
      extent,
    )?;

    Ok(Self {
      layout,
      current: initial,
      player_shader,
      bullets_shader,
      old: None,
    })
  }

  // create a new pipeline and mark the other as old
  pub fn recreate(
    &mut self,
    device: &ash::Device,
    cache: vk::PipelineCache,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
  ) -> Result<(), PipelineCreationError> {
    assert!(self.old.is_none());

    let mut new = Self::create_with_base(
      device,
      self.layout,
      cache,
      self.current,
      self.player_shader,
      self.bullets_shader,
      render_pass,
      extent,
    )?;

    let old = {
      mem::swap(&mut self.current, &mut new);
      new
    };

    self.old = Some(old);
    Ok(())
  }

  pub fn revert_recreate(&mut self, device: &ash::Device) {
    unsafe {
      self.current.destroy_self(device);
    }
    let mut temp = None;
    mem::swap(&mut self.old, &mut temp);
    self.current = temp.unwrap();
  }

  // destroy old pipeline once it stops being used
  pub unsafe fn destroy_old(&mut self, device: &ash::Device) {
    if let Some(old) = self.old {
      old.destroy_self(device);
      self.old = None;
    }
  }

  fn create_layout(
    device: &ash::Device,
    descriptor_pool: &DescriptorPool,
  ) -> Result<vk::PipelineLayout, OutOfMemoryError> {
    let push_constant_range = vk::PushConstantRange {
      stage_flags: vk::ShaderStageFlags::VERTEX,
      offset: 0,
      size: size_of::<SpritePushConstants>() as u32,
    };
    let layout_create_info = vk::PipelineLayoutCreateInfo {
      s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineLayoutCreateFlags::empty(),
      set_layout_count: 1,
      p_set_layouts: &descriptor_pool.texture_layout,
      push_constant_range_count: 1,
      p_push_constant_ranges: &push_constant_range,
      _marker: PhantomData,
    };
    unsafe { device.create_pipeline_layout(&layout_create_info, None) }
      .map_err(OutOfMemoryError::from)
  }

  fn create_with_base(
    device: &ash::Device,
    layout: vk::PipelineLayout,
    cache: vk::PipelineCache,
    old: GraphicsPipelinesList,
    player_shader: shaders::player::Shader,
    bullets_shader: shaders::bullets::Shader,
    render_pass: vk::RenderPass,
    extent: vk::Extent2D,
  ) -> Result<GraphicsPipelinesList, PipelineCreationError> {
    let player_shader_stages = player_shader.get_pipeline_shader_creation_info();
    let bullets_shader_stages = bullets_shader.get_pipeline_shader_creation_info();

    let player_vertex_input_state = vertex_input_state_create_info!(Vertex);
    let player_vertex_input_state = player_vertex_input_state.get();

    let bullets_vertex_input_state = vertex_input_state_create_info!(Vertex, Bullet);
    let bullets_vertex_input_state = bullets_vertex_input_state.get();

    let input_assembly_state = triangle_input_assembly_state();

    // full image viewport and scissor
    let viewport = [vk::Viewport {
      x: 0.0,
      y: 0.0,
      width: extent.width as f32,
      height: extent.height as f32,
      min_depth: 0.0,
      max_depth: 1.0,
    }];
    let scissor = [vk::Rect2D {
      offset: vk::Offset2D { x: 0, y: 0 },
      extent,
    }];
    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
      .scissors(&scissor)
      .viewports(&viewport);

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

      // always set alpha to one
      src_alpha_blend_factor: vk::BlendFactor::ONE,
      dst_alpha_blend_factor: vk::BlendFactor::ONE,
      alpha_blend_op: vk::BlendOp::ADD,
    };
    let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
      s_type: vk::StructureType::PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineColorBlendStateCreateFlags::empty(),
      logic_op_enable: vk::FALSE,
      logic_op: vk::LogicOp::COPY, // disabled
      attachment_count: 1,
      p_attachments: &attachment_state,
      blend_constants: [0.0, 0.0, 0.0, 0.0],
      _marker: PhantomData,
    };

    let player_create_info = {
      let mut flags = vk::PipelineCreateFlags::ALLOW_DERIVATIVES;
      if !old.player.is_null() {
        flags = flags.bitor(vk::PipelineCreateFlags::DERIVATIVE)
      }
      vk::GraphicsPipelineCreateInfo {
        s_type: vk::StructureType::GRAPHICS_PIPELINE_CREATE_INFO,
        p_next: ptr::null(),
        flags,
        stage_count: player_shader_stages.len() as u32,
        p_stages: player_shader_stages.as_ptr(),
        p_vertex_input_state: player_vertex_input_state,
        p_input_assembly_state: &input_assembly_state,
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
        base_pipeline_handle: old.player,
        base_pipeline_index: -1, // -1 for null
        _marker: PhantomData,
      }
    };

    let bullets_create_info = {
      let mut ci = player_create_info.clone();

      let mut flags = vk::PipelineCreateFlags::ALLOW_DERIVATIVES;
      if !old.bullets.is_null() {
        flags = flags.bitor(vk::PipelineCreateFlags::DERIVATIVE)
      }
      ci.flags = flags;

      ci.stage_count = bullets_shader_stages.len() as u32;
      ci.p_stages = bullets_shader_stages.as_ptr();
      ci.p_vertex_input_state = bullets_vertex_input_state;
      ci.base_pipeline_handle = old.bullets;
      ci
    };

    let vec = unsafe {
      device
        .create_graphics_pipelines(cache, &[player_create_info, bullets_create_info], None)
        .map_err(|incomplete| incomplete.1)
        .map_err(|vkerr| match vkerr {
          vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
            PipelineCreationError::from(OutOfMemoryError::from(vkerr))
          }
          vk::Result::ERROR_INVALID_SHADER_NV => PipelineCreationError::CompilationFailed,
          _ => panic!(),
        })?
    };

    Ok(GraphicsPipelinesList {
      player: vec[0],
      bullets: vec[1],
    })
  }
}

const fn triangle_input_assembly_state<'a>() -> vk::PipelineInputAssemblyStateCreateInfo<'a> {
  vk::PipelineInputAssemblyStateCreateInfo {
    s_type: vk::StructureType::PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
    flags: vk::PipelineInputAssemblyStateCreateFlags::empty(),
    p_next: ptr::null(),
    // defines that there exists a special value that restarts the assembly
    primitive_restart_enable: vk::FALSE,
    topology: vk::PrimitiveTopology::TRIANGLE_LIST,
    _marker: PhantomData,
  }
}

// rasterization with no depth and no culling
const fn no_depth_rasterization_state<'a>() -> vk::PipelineRasterizationStateCreateInfo<'a> {
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
    _marker: PhantomData,
  }
}

const fn no_multisample_state<'a>() -> vk::PipelineMultisampleStateCreateInfo<'a> {
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
    _marker: PhantomData,
  }
}

impl DeviceManuallyDestroyed for GraphicsPipelinesList {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.player.destroy_self(device);
    self.bullets.destroy_self(device);
  }
}

impl DeviceManuallyDestroyed for GraphicsPipelines {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    if let Some(old) = self.old {
      old.destroy_self(device);
    }
    self.current.destroy_self(device);
    self.layout.destroy_self(device);

    // can be unloaded any time
    self.player_shader.destroy_self(device);
    self.bullets_shader.destroy_self(device);
  }
}
