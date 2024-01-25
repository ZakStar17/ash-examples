use ash::vk::{self, DebugUtilsMessengerCreateInfoEXT};

use std::{ffi::CStr, os::raw::c_void, ptr};

use crate::{utility, VALIDATION_LAYERS};

#[derive(Debug)]
struct LayerProperties {
  name: String,
  _description: String,
  _implementation_version: String,
}

impl PartialEq for LayerProperties {
  fn eq(&self, other: &Self) -> bool {
    self.name.eq(&other.name)
  }
}

impl PartialOrd for LayerProperties {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    self.name.partial_cmp(&other.name)
  }
}

impl Eq for LayerProperties {}

impl Ord for LayerProperties {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.name.cmp(&other.name)
  }
}

// returns a subset of VALIDATION_LAYERS that are available
pub fn get_supported_validation_layers(entry: &ash::Entry) -> Box<[&'static CStr]> {
  log::info!("Checking for validation layers");

  // supposedly only fails if there is no available memory
  let properties: Vec<vk::LayerProperties> = entry.enumerate_instance_layer_properties().unwrap();

  let mut all: Vec<LayerProperties> = properties
    .iter()
    .filter_map(
      |props| match utility::i8_array_to_string(&props.layer_name) {
        Ok(s) => Some((props, s)),
        Err(_) => {
          log::warn!(
          "There exists an available validation layer with an invalid name that couldn't be decoded"
        );
          None
        }
      },
    )
    .map(|(props, name)| LayerProperties {
      name,
      _description: utility::i8_array_to_string(&props.description)
        .unwrap_or(String::from("<Couldn't be decoded>")),
      _implementation_version: utility::parse_vulkan_api_version(props.implementation_version),
    })
    .collect();

  log::debug!("System validation layers: {:#?}", all);

  let available = utility::in_slice(
    all.as_mut_slice(),
    &mut VALIDATION_LAYERS.clone().into_iter(),
    |av, req| av.name.as_str().cmp(req.to_str().unwrap()),
  );

  if available.len() != VALIDATION_LAYERS.len() {
    let unavailable: Vec<&&CStr> = VALIDATION_LAYERS
      .iter()
      .filter(|s| !available.contains(s))
      .collect();
    log::error!(
      "Some requested validation layers are not available: {:?}",
      unavailable
    );
  }

  available
}

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
  pub fn setup(
    entry: &ash::Entry,
    instance: &ash::Instance,
    create_info: DebugUtilsMessengerCreateInfoEXT,
  ) -> Self {
    let loader = ash::extensions::ext::DebugUtils::new(entry, instance);

    log::debug!("Creating debug utils messenger");
    let messenger = unsafe {
      loader
        .create_debug_utils_messenger(&create_info, None)
        .expect("Failed to create debug utils")
    };

    Self { loader, messenger }
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
