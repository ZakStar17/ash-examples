use ash::vk::{self, DebugUtilsMessengerCreateInfoEXT};

use std::{ffi::CStr, os::raw::c_void, ptr};

use crate::{utility, VALIDATION_LAYERS};

// returns a list of supported and unsupported instance layers
fn filter_supported(
  available: Vec<vk::LayerProperties>,
) -> (Vec<&'static CStr>, Vec<&'static CStr>) {
  VALIDATION_LAYERS.into_iter().partition(|&req| {
    available
      .iter()
      .any(|av| unsafe { utility::i8_array_as_cstr(&av.layer_name) }.unwrap() == req)
  })
}

// returns a subset of VALIDATION_LAYERS that are available
pub fn get_supported_validation_layers(
  entry: &ash::Entry,
) -> Result<Box<[&'static CStr]>, vk::Result> {
  log::info!("Querying Vulkan instance layers");
  let (available, unavailable) = filter_supported(entry.enumerate_instance_layer_properties()?);

  if !unavailable.is_empty() {
    log::error!(
      "Some requested validation layers are not available: {:?}",
      unavailable
    );
  }

  Ok(available.into_boxed_slice())
}

// can be extensively customized
unsafe extern "system" fn vulkan_debug_utils_callback(
  message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
  message_type: vk::DebugUtilsMessageTypeFlagsEXT,
  p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
  _p_user_data: *mut c_void,
) -> vk::Bool32 {
  let types = match message_type {
    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "[General]",
    vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "[Performance]",
    vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "[Validation]",
    _ => "[Unknown]",
  };
  let message = CStr::from_ptr((*p_callback_data).p_message);
  let message = format!("{} {}", types, message.to_str().unwrap());
  match message_severity {
    vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => log::debug!("{message}"),
    vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => log::warn!("{message}"),
    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => log::error!("{message}"),
    vk::DebugUtilsMessageSeverityFlagsEXT::INFO => log::info!("{message}"),
    _ => log::warn!("<Unknown>: {message}"),
  }

  vk::FALSE
}

pub struct DebugUtils {
  loader: ash::extensions::ext::DebugUtils,
  messenger: vk::DebugUtilsMessengerEXT,
}

impl DebugUtils {
  pub fn create(
    entry: &ash::Entry,
    instance: &ash::Instance,
    create_info: DebugUtilsMessengerCreateInfoEXT,
  ) -> Result<Self, vk::Result> {
    let loader = ash::extensions::ext::DebugUtils::new(entry, instance);

    let messenger = unsafe { loader.create_debug_utils_messenger(&create_info, None)? };

    Ok(Self { loader, messenger })
  }

  pub fn get_debug_messenger_create_info() -> vk::DebugUtilsMessengerCreateInfoEXT {
    vk::DebugUtilsMessengerCreateInfoEXT {
      s_type: vk::StructureType::DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT,
      p_next: ptr::null(),
      flags: vk::DebugUtilsMessengerCreateFlagsEXT::empty(),
      message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
        | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
        | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
      message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
      pfn_user_callback: Some(vulkan_debug_utils_callback),
      p_user_data: ptr::null_mut(),
    }
  }

  pub unsafe fn destroy_self(&mut self) {
    self
      .loader
      .destroy_debug_utils_messenger(self.messenger, None);
  }
}
