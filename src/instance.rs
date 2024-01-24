use ash::vk;
use std::ptr::{self};

use crate::{utility, APPLICATION_NAME, APPLICATION_VERSION, TARGET_API_VERSION};

// Checks if all required extensions exist and are supported by the host system
// If found returns a list of required but not available extensions as an error
fn check_instance_extension_support<'a>(
  entry: &ash::Entry,
  extensions: &'a Vec<&'a str>,
) -> Result<(), Vec<&'a &'a str>> {
  let required = extensions;
  log::info!(
    "Required Instance extensions by the application: {:?}",
    required
  );

  let mut available: Vec<String> = entry
    .enumerate_instance_extension_properties(None)
    .unwrap() // should only fail if out of memory
    .iter()
    .filter_map(
      |props| match utility::i8_array_to_string(&props.extension_name) {
        Ok(s) => Some(s),
        Err(_) => {
          log::warn!(
            "There exists an available extension with an invalid name that couldn't be decoded"
          );
          None
        }
      },
    )
    .collect();

  log::debug!("Available Instance extensions: {:?}", available);

  let unavailable = utility::not_in_string_slice(available.as_mut_slice(), &mut required.iter());
  if unavailable.is_empty() {
    Ok(())
  } else {
    Err(unavailable)
  }
}

fn check_target_api_version(entry: &ash::Entry) {
  let max_supported_version = match entry.try_enumerate_instance_version() {
    // Vulkan 1.1+
    Ok(opt) => match opt {
      Some(version) => version,
      None => vk::API_VERSION_1_0,
    },
    // Vulkan 1.0
    Err(_) => vk::API_VERSION_1_0,
  };

  log::info!(
    "Vulkan library max supported version: {}",
    utility::parse_vulkan_api_version(max_supported_version)
  );

  if max_supported_version < TARGET_API_VERSION {
    panic!("Vulkan implementation API maximum supported version is less than the one targeted by the application.");
  }
}

fn get_app_info() -> vk::ApplicationInfo {
  vk::ApplicationInfo {
    s_type: vk::StructureType::APPLICATION_INFO,
    api_version: TARGET_API_VERSION,
    p_application_name: APPLICATION_NAME.as_ptr(),
    application_version: APPLICATION_VERSION,
    p_engine_name: ptr::null(),
    engine_version: vk::make_api_version(0, 1, 0, 0),
    p_next: ptr::null(),
  }
}

#[cfg(feature = "vl")]
pub fn create_instance(
  entry: &ash::Entry,
) -> (ash::Instance, crate::validation_layers::DebugUtils) {
  use std::{ffi::c_void, ptr::addr_of};

  use crate::{
    validation_layers::{self, DebugUtils},
    ADDITIONAL_VALIDATION_FEATURES,
  };

  check_target_api_version(entry);

  let required_extensions = vec![ash::extensions::ext::DebugUtils::name()];
  let required_extensions_str = required_extensions
    .iter()
    .map(|cs| cs.to_str().unwrap())
    .collect();
  check_instance_extension_support(entry, &required_extensions_str).unwrap_or_else(|unavailable| {
    panic!(
      "Some unavailable Instance extensions are strictly required: {:?}",
      unavailable
    )
  });
  // required to be alive until the end of instance creation
  let required_extensions_ptr: Vec<*const i8> = required_extensions
    .iter()
    .map(|v| v.as_ptr() as *const i8)
    .collect();

  let app_info = get_app_info();

  let validation_layers = validation_layers::get_supported_validation_layers(&entry);
  // valid for as long as "validation_layers"
  let vl_pointers: Vec<*const std::ffi::c_char> =
    validation_layers.iter().map(|name| name.as_ptr()).collect();

  // required to be passed in instance creation p_next chain
  let debug_create_info = DebugUtils::get_debug_messenger_create_info();

  // you can enable/disable additional features by passing a ValidationFeaturesEXT struct
  let additional_features = vk::ValidationFeaturesEXT {
    s_type: vk::StructureType::VALIDATION_FEATURES_EXT,
    p_next: addr_of!(debug_create_info) as *const c_void,
    enabled_validation_feature_count: ADDITIONAL_VALIDATION_FEATURES.len() as u32,
    p_enabled_validation_features: ADDITIONAL_VALIDATION_FEATURES.as_ptr(),
    disabled_validation_feature_count: 0,
    p_disabled_validation_features: ptr::null(),
  };

  let create_info = vk::InstanceCreateInfo {
    s_type: vk::StructureType::INSTANCE_CREATE_INFO,
    p_next: addr_of!(additional_features) as *const c_void,
    p_application_info: &app_info,
    pp_enabled_layer_names: vl_pointers.as_ptr(),
    enabled_layer_count: vl_pointers.len() as u32,
    pp_enabled_extension_names: required_extensions_ptr.as_ptr(),
    enabled_extension_count: required_extensions_ptr.len() as u32,
    flags: vk::InstanceCreateFlags::empty(),
  };

  log::debug!("Creating Instance");
  let instance: ash::Instance = unsafe {
    entry
      .create_instance(&create_info, None)
      .expect("Failed to create Instance")
  };

  log::debug!("Creating Debug Utils");
  let debug_utils = DebugUtils::setup(&entry, &instance, debug_create_info);

  (instance, debug_utils)
}

#[cfg(not(feature = "vl"))]
pub fn create_instance(entry: &ash::Entry) -> ash::Instance {
  check_target_api_version(entry);

  let required_extensions = vec![];
  check_instance_extension_support(entry, &required_extensions).unwrap_or_else(|unavailable| {
    panic!(
      "Some unavailable Instance extensions are strictly required: {:?}",
      unavailable
    )
  });
  // required to be alive until the end of instance creation
  let required_extensions_ptr: Vec<*const i8> = required_extensions
    .iter()
    .map(|v| v.as_ptr() as *const i8)
    .collect();

  let app_info = get_app_info();

  let create_info = vk::InstanceCreateInfo {
    s_type: vk::StructureType::INSTANCE_CREATE_INFO,
    p_next: ptr::null(),
    p_application_info: &app_info,
    pp_enabled_layer_names: ptr::null(),
    enabled_layer_count: 0,
    pp_enabled_extension_names: required_extensions_ptr.as_ptr(),
    enabled_extension_count: required_extensions_ptr.len() as u32,
    flags: vk::InstanceCreateFlags::empty(),
  };

  log::debug!("Creating Instance");
  unsafe {
    entry
      .create_instance(&create_info, None)
      .expect("Failed to create Instance")
  }
}
