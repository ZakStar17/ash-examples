use ash::vk::{self, DebugUtilsMessengerCreateInfoEXT};

use std::{
  ffi::{CStr, CString},
  os::raw::c_void,
  ptr,
};

macro_rules! cstr {
  ( $s:literal ) => {{
    unsafe { std::mem::transmute::<_, &CStr>(concat!($s, "\0")) }
  }};
}

// array of validation layers that should be loaded
// validation layers names should be valid cstrings (not contain null bytes nor invalid characters)
const VALIDATION_LAYERS: [&'static CStr; 1] = [cstr!("VK_LAYER_KHRONOS_validation")];

fn check_validation_layers_support(entry: &ash::Entry) -> Result<(), Vec<CString>> {
  let properties: Vec<vk::LayerProperties> = entry.enumerate_instance_layer_properties().unwrap();
  let mut available: Vec<&CStr> = properties
    .iter()
    .map(|p| {
      let i8slice: &[i8] = &p.layer_name;
      let slice: &[u8] =
        unsafe { std::slice::from_raw_parts(i8slice.as_ptr() as *const u8, i8slice.len()) };
      CStr::from_bytes_until_nul(slice).expect("Failed to read system available validation layers")
    })
    .collect();
  available.sort();

  log::debug!("System available validation layers: {:?}", available);
  log::info!("Checking for validation layers ({:?})", VALIDATION_LAYERS);

  let mut unavailable = Vec::new();
  for name in VALIDATION_LAYERS {
    if let Err(_) = available.binary_search_by(|&av| av.cmp(name)) {
      unavailable.push(name.to_owned());
    }
  }

  if unavailable.is_empty() {
    Ok(())
  } else {
    Err(unavailable)
  }
}

// returns a vec of wanted validation layers that are possible to load
pub fn get_validation_layers(entry: &ash::Entry) -> Vec<&'static CStr> {
  if let Err(mut unavailable) = check_validation_layers_support(entry) {
    unavailable.sort();
    log::error!(
      "Some validation layers could not be found and will not be loaded: {:?}",
      unavailable
    );
    VALIDATION_LAYERS
      .into_iter()
      .filter(|&name| {
        unavailable
          .binary_search_by(|s| s.as_c_str().cmp(name))
          .is_err()
      })
      .collect()
  } else {
    VALIDATION_LAYERS.to_vec()
  }
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
