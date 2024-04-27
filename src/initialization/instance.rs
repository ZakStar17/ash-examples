use ash::vk;
use std::{
  ffi::{c_char, c_void, CStr},
  marker::PhantomData,
  ptr,
};

use crate::{
  errors::OutOfMemoryError, utility, APPLICATION_NAME, APPLICATION_VERSION, TARGET_API_VERSION,
};

#[derive(thiserror::Error, Debug)]
pub enum InstanceCreationError {
  #[error("Vulkan implementation API maximum supported version ({0}) is less than the one targeted by the application ({1})")]
  UnsupportedApiVersion(String, String),

  #[error("Missing instance extension \"{0}\"")]
  MissingExtension(String),
  // validation layers will be skipped if not available
  #[error("Missing instance layer \"{0}\"")]
  MissingLayer(String),

  #[error("")]
  OutOfMemory(#[from] OutOfMemoryError),

  #[error("Failed to create an instance because of an unknown reason")]
  Failed,
}

fn check_api_version(entry: &ash::Entry) -> Result<(), InstanceCreationError> {
  let max_supported_version = match unsafe { entry.try_enumerate_instance_version() } {
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
    return Err(InstanceCreationError::UnsupportedApiVersion(
      utility::parse_vulkan_api_version(max_supported_version),
      utility::parse_vulkan_api_version(TARGET_API_VERSION),
    ));
  }

  Ok(())
}

fn get_app_info<'a>() -> vk::ApplicationInfo<'a> {
  vk::ApplicationInfo {
    s_type: vk::StructureType::APPLICATION_INFO,
    api_version: TARGET_API_VERSION,
    p_application_name: APPLICATION_NAME.as_ptr(),
    application_version: APPLICATION_VERSION,
    p_engine_name: ptr::null(),
    engine_version: vk::make_api_version(0, 1, 0, 0),
    p_next: ptr::null(),
    _marker: PhantomData,
  }
}

#[cfg(feature = "vl")]
pub fn create_instance(
  entry: &ash::Entry,
) -> Result<
  (
    ash::Instance,
    crate::initialization::validation_layers::DebugUtils,
  ),
  InstanceCreationError,
> {
  use std::ptr::addr_of;

  use crate::{
    initialization::validation_layers::{self, DebugUtils},
    ADDITIONAL_VALIDATION_FEATURES,
  };

  let app_info = get_app_info();

  let extensions = vec![ash::ext::debug_utils::NAME.as_ptr()];

  let layers_str = validation_layers::get_supported_validation_layers(&entry)
    .map_err(|err| InstanceCreationError::OutOfMemory(err.into()))?;
  let layers: Vec<*const c_char> = layers_str.iter().map(|name| name.as_ptr()).collect();

  let debug_create_info = DebugUtils::get_debug_messenger_create_info();

  // enable/disable some validation features by passing a ValidationFeaturesEXT struct
  let additional_features = vk::ValidationFeaturesEXT {
    s_type: vk::StructureType::VALIDATION_FEATURES_EXT,
    p_next: addr_of!(debug_create_info) as *const c_void,
    enabled_validation_feature_count: ADDITIONAL_VALIDATION_FEATURES.len() as u32,
    p_enabled_validation_features: ADDITIONAL_VALIDATION_FEATURES.as_ptr(),
    disabled_validation_feature_count: 0,
    p_disabled_validation_features: ptr::null(),
    _marker: PhantomData,
  };

  let instance = create_instance_checked(
    entry,
    app_info,
    &extensions,
    &layers,
    addr_of!(additional_features) as *const c_void,
  )?;

  log::debug!("Creating Debug Utils");
  let debug_utils = DebugUtils::create(&entry, &instance, debug_create_info)?;

  Ok((instance, debug_utils))
}

#[cfg(not(feature = "vl"))]
pub fn create_instance(entry: &ash::Entry) -> Result<ash::Instance, InstanceCreationError> {
  check_api_version(entry)?;

  let app_info = get_app_info();
  let extensions = [];
  let layers = [];
  create_instance_checked(entry, app_info, &extensions, &layers, ptr::null())
}

// check if extensions are layers are present and then create a vk instance
fn create_instance_checked(
  entry: &ash::Entry,
  app_info: vk::ApplicationInfo,
  extensions: &[*const c_char],
  layers: &[*const c_char],
  p_next: *const c_void,
) -> Result<ash::Instance, InstanceCreationError> {
  check_api_version(entry)?;

  // check that all extensions are available
  {
    let available = unsafe { entry.enumerate_instance_extension_properties(None) }
      .map_err(|err| InstanceCreationError::OutOfMemory(err.into()))?;
    for &ptr in extensions {
      let extension = unsafe { CStr::from_ptr(ptr) };
      if !available
        .iter()
        .filter_map(|av| av.extension_name_as_c_str().ok())
        .any(|av| av == extension)
      {
        return Err(InstanceCreationError::MissingExtension(String::from(
          extension.to_str().unwrap(),
        )));
      }
    }
  };

  // check that all layers are available
  {
    let available = unsafe { entry.enumerate_instance_layer_properties() }
      .map_err(|err| InstanceCreationError::OutOfMemory(err.into()))?;
    for &ptr in layers {
      let layer = unsafe { CStr::from_ptr(ptr) };
      if !available
        .iter()
        .filter_map(|av| av.layer_name_as_c_str().ok())
        .any(|av| av == layer)
      {
        return Err(InstanceCreationError::MissingLayer(String::from(
          layer.to_str().unwrap(),
        )));
      }
    }
  };

  let mut create_info = vk::InstanceCreateInfo::default()
    .application_info(&app_info)
    .enabled_extension_names(extensions)
    .enabled_layer_names(layers);
  create_info.p_next = p_next;

  log::debug!("Creating Instance");
  let instance: ash::Instance =
    unsafe { entry.create_instance(&create_info, None) }.map_err(|err| match err {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
        InstanceCreationError::OutOfMemory(OutOfMemoryError::from(err))
      }
      vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        InstanceCreationError::OutOfMemory(OutOfMemoryError::from(err))
      }
      vk::Result::ERROR_INITIALIZATION_FAILED => InstanceCreationError::Failed,
      // other results have been checked
      _ => panic!(),
    })?;

  Ok(instance)
}
