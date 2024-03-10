use ash::vk;
use std::{
  ffi::CStr,
  ptr::{self},
};

use crate::{utility, APPLICATION_NAME, APPLICATION_VERSION, TARGET_API_VERSION};

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

// Returns a subset of unavailable extensions
fn filter_unavailable_extensions<'a>(
  available: Vec<vk::ExtensionProperties>,
  required: &'a [&'a CStr],
) -> Box<[&'a &'a CStr]> {
  required
    .iter()
    .filter(|&req| {
      !available
        .iter()
        .any(|av| unsafe { utility::i8_array_as_cstr(&av.extension_name) }.unwrap() == *req)
    })
    .collect()
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
) -> Result<(ash::Instance, crate::validation_layers::DebugUtils), vk::Result> {
  use std::{ffi::c_void, ptr::addr_of};

  use crate::{
    validation_layers::{self, DebugUtils},
    ADDITIONAL_VALIDATION_FEATURES,
  };

  check_target_api_version(entry);

  let required_extensions = vec![ash::extensions::ext::DebugUtils::name()];
  let unavailable_extensions = filter_unavailable_extensions(
    entry.enumerate_instance_extension_properties(None)?,
    required_extensions.as_slice(),
  );
  if !unavailable_extensions.is_empty() {
    panic!(
      "Some instance extensions are not available: {:?}",
      unavailable_extensions
    )
  };
  let required_extensions_ptrs: Vec<*const i8> = required_extensions
    .iter()
    .map(|v| v.as_ptr() as *const i8)
    .collect();

  let app_info = get_app_info();

  // valid until the end of scope
  let validation_layers = validation_layers::get_supported_validation_layers(&entry)?;
  let vl_pointers: Vec<*const std::ffi::c_char> =
    validation_layers.iter().map(|name| name.as_ptr()).collect();

  let debug_create_info = DebugUtils::get_debug_messenger_create_info();

  // enable/disable some validation features by passing a ValidationFeaturesEXT struct
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
    pp_enabled_extension_names: required_extensions_ptrs.as_ptr(),
    enabled_extension_count: required_extensions_ptrs.len() as u32,
    flags: vk::InstanceCreateFlags::empty(),
  };

  log::debug!("Creating Instance");
  let instance: ash::Instance = unsafe { entry.create_instance(&create_info, None)? };

  log::debug!("Creating Debug Utils");
  let debug_utils = DebugUtils::create(&entry, &instance, debug_create_info)?;

  Ok((instance, debug_utils))
}

#[cfg(not(feature = "vl"))]
pub fn create_instance(entry: &ash::Entry) -> Result<ash::Instance, vk::Result> {
  check_target_api_version(entry);

  let required_extensions = vec![ash::extensions::ext::DebugUtils::name()];
  let unavailable_extensions = filter_unavailable_extensions(
    entry.enumerate_instance_extension_properties(None)?,
    required_extensions.as_slice(),
  );
  if !unavailable_extensions.is_empty() {
    panic!(
      "Some instance extensions are not available: {:?}",
      unavailable_extensions
    )
  };
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
  unsafe { entry.create_instance(&create_info, None) }
}
