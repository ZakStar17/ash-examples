use ash::vk;
use std::{ffi::CString, ptr};

#[cfg(feature = "vulkan_vl")]
use std::os::raw::{c_char, c_void};

use crate::utility;

const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

// Checks if all required extensions exist and are supported by the host system
// If found returns a list of required but not available extensions as an error
fn test_instance_extension_support(
  entry: &ash::Entry,
  extensions: &Vec<*const i8>,
) -> Result<(), Vec<String>> {
  let required: Vec<&str> = extensions
    .iter()
    .map(|x| {
      let rust_id = unsafe { std::ffi::CStr::from_ptr(*x) };
      rust_id.to_str().unwrap()
    })
    .collect();
  log::info!("Required instance extensions: {:?}", required);

  let mut available: Vec<String> = entry
    .enumerate_instance_extension_properties(None)
    .unwrap()
    .iter()
    .filter_map(|x| match utility::i8_array_to_string(&x.extension_name) {
      Ok(s) => Some(s),
      Err(_) => {
        log::warn!(
          "There exists an available extension with an invalid name that could not be decoded"
        );
        None
      }
    })
    .collect();
  available.sort();

  log::debug!("Available instance extensions: {:?}", available);

  let mut unavailable = Vec::new();
  for name in required.into_iter() {
    if available
      .binary_search_by(|av| av.as_str().cmp(name))
      .is_err()
    {
      unavailable.push(name.to_string());
    }
  }

  if unavailable.is_empty() {
    Ok(())
  } else {
    Err(unavailable)
  }
}

pub fn create_instance(
  entry: &ash::Entry,
  #[cfg(feature = "vulkan_vl")] vl_pointers: &Vec<*const c_char>,
  #[cfg(feature = "vulkan_vl")] debug_create_info: &vk::DebugUtilsMessengerCreateInfoEXT,
) -> ash::Instance {
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
    "Vulkan library max supported version: {}.{}.{}",
    vk::api_version_major(max_supported_version),
    vk::api_version_minor(max_supported_version),
    vk::api_version_patch(max_supported_version)
  );

  if max_supported_version < TARGET_API_VERSION {
    panic!("Vulkan implementation API maximum supported version is less than the one targeted by the application");
  }

  let app_name = CString::new("Ash By Example").unwrap();
  let app_info = vk::ApplicationInfo {
    s_type: vk::StructureType::APPLICATION_INFO,
    api_version: vk::API_VERSION_1_3,
    p_application_name: app_name.as_ptr(),
    application_version: vk::make_api_version(0, 1, 0, 0),
    p_engine_name: ptr::null(),
    engine_version: vk::make_api_version(0, 1, 0, 0),
    p_next: ptr::null(),
  };

  #[allow(unused_mut)]
  let mut required_extensions = Vec::with_capacity(1);
  #[cfg(feature = "vulkan_vl")]
  required_extensions.push(ash::extensions::ext::DebugUtils::name().as_ptr());
  test_instance_extension_support(entry, &required_extensions).unwrap_or_else(|unavailable| {
    panic!(
      "Some unavailable instance extensions are strictly required: {:?}",
      unavailable
    )
  });

  #[allow(unused_mut)]
  let mut create_info = vk::InstanceCreateInfo {
    s_type: vk::StructureType::INSTANCE_CREATE_INFO,
    p_next: ptr::null(),
    p_application_info: &app_info,
    pp_enabled_layer_names: ptr::null(),
    enabled_layer_count: 0,
    pp_enabled_extension_names: required_extensions.as_ptr(),
    enabled_extension_count: required_extensions.len() as u32,
    flags: vk::InstanceCreateFlags::empty(),
  };

  #[cfg(feature = "vulkan_vl")]
  {
    create_info.p_next =
      debug_create_info as *const vk::DebugUtilsMessengerCreateInfoEXT as *const c_void;
    create_info.pp_enabled_layer_names = vl_pointers.as_ptr();
    create_info.enabled_layer_count = vl_pointers.len() as u32;
  }

  log::debug!("Creating instance");
  let instance: ash::Instance = unsafe {
    entry
      .create_instance(&create_info, None)
      .expect("Failed to create instance")
  };

  instance
}
