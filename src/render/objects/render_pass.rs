use std::ptr;

use ash::vk;

pub fn create_render_pass(device: &ash::Device, surface_format: vk::Format) -> vk::RenderPass {
  let image_attachment = vk::AttachmentDescription {
    flags: vk::AttachmentDescriptionFlags::empty(),
    format: surface_format,
    samples: vk::SampleCountFlags::TYPE_1,
    load_op: vk::AttachmentLoadOp::CLEAR,
    store_op: vk::AttachmentStoreOp::STORE,
    stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
    stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
    initial_layout: vk::ImageLayout::UNDEFINED,
    final_layout: vk::ImageLayout::PRESENT_SRC_KHR, // layout after render pass finishes
  };

  let attachment_ref = vk::AttachmentReference {
    attachment: 0,
    layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
  };

  let image_subpass = vk::SubpassDescription {
    flags: vk::SubpassDescriptionFlags::empty(),
    pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
    input_attachment_count: 0,
    p_input_attachments: ptr::null(),
    // output attachments
    color_attachment_count: 1,
    p_color_attachments: &attachment_ref,
    p_resolve_attachments: ptr::null(),
    p_depth_stencil_attachment: ptr::null(),
    preserve_attachment_count: 0,
    p_preserve_attachments: ptr::null(),
  };

  let dependencies = [
    // change access flags to attachment before subpass begins
    vk::SubpassDependency {
      src_subpass: vk::SUBPASS_EXTERNAL,
      dst_subpass: 0, // image_subpass
      src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
      dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
      src_access_mask: vk::AccessFlags::NONE,
      dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
      dependency_flags: vk::DependencyFlags::empty(),
    },
  ];

  let create_info = vk::RenderPassCreateInfo {
    s_type: vk::StructureType::RENDER_PASS_CREATE_INFO,
    flags: vk::RenderPassCreateFlags::empty(),
    p_next: ptr::null(),
    attachment_count: 1,
    p_attachments: &image_attachment,
    subpass_count: 1,
    p_subpasses: &image_subpass,
    dependency_count: dependencies.len() as u32,
    p_dependencies: dependencies.as_ptr(),
  };
  unsafe {
    device
      .create_render_pass(&create_info, None)
      .expect("Failed to create render pass!")
  }
}

pub fn create_framebuffer(
  device: &ash::Device,
  render_pass: vk::RenderPass,
  image_view: vk::ImageView,
  extent: vk::Extent2D,
) -> vk::Framebuffer {
  let create_info = vk::FramebufferCreateInfo {
    s_type: vk::StructureType::FRAMEBUFFER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::FramebufferCreateFlags::empty(),
    render_pass,
    attachment_count: 1,
    p_attachments: &image_view,
    width: extent.width,
    height: extent.height,
    layers: 1,
  };
  unsafe {
    device
      .create_framebuffer(&create_info, None)
      .expect("Failed to create framebuffer")
  }
}
