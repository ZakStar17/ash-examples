use std::ffi::CStr;

#[cfg(feature = "vulkan_vl")]
use validation_layers::{get_validation_layers, DebugUtils};

use crate::{physical_device::select_physical_device, logical_device::create_logical_device};

mod instance;
mod logical_device;
mod physical_device;
mod utility;

#[cfg(feature = "vulkan_vl")]
mod validation_layers;

#[cfg(all(feature = "link_vulkan", feature = "load_vulkan"))]
compile_error!(
  "\
    Features \"link_vulkan\" and \"load_vulkan\" \
    were included at the same time. \
    Choose between \"load_vulkan\" to load the Vulkan library \
    at runtime or \"link_vulkan\" to link it at compile time."
);

macro_rules! cstr {
  ( $s:literal ) => {{
    unsafe { std::mem::transmute::<_, &CStr>(concat!($s, "\0")) }
  }};
}

pub const DEVICE_EXTENSIONS: [&'static str; 0] = [];

// array of validation layers that should be loaded
// validation layers names should be valid cstrings (not contain null bytes nor invalid characters)
pub const VALIDATION_LAYERS: [&'static CStr; 1] = [cstr!("VK_LAYER_KHRONOS_validation")];

#[allow(unreachable_code)]
unsafe fn get_entry() -> ash::Entry {
  #[cfg(feature = "link_vulkan")]
  return ash::Entry::linked();
  #[cfg(feature = "load_vulkan")]
  return match ash::Entry::load() {
    Ok(entry) => entry,
    Err(err) => match err {
      ash::LoadingError::MissingEntryPoint(missing_entry_error) => {
        panic!(
          "Missing entry point when loading Vulkan library: {}",
          missing_entry_error
        )
      }
      ash::LoadingError::LibraryLoadFailure(load_error) => {
        panic!("Failed to load Vulkan Library: {:?}", load_error)
      }
    },
  };
  panic!(
    "No compile feature was included for accessing the Vulkan library.\n\
    Choose between \"load_vulkan\" to load the Vulkan library \
    at runtime or \"link_vulkan\" to link it at compile time."
  );
}

fn main() {
  env_logger::init();

  let entry: ash::Entry = unsafe { get_entry() };

  #[cfg(feature = "vulkan_vl")]
  let (_validation_layers, vl_pointers) = {
    let validation_layers = get_validation_layers(&entry);
    // valid for as long as "validation_layers"
    let vl_pointers: Vec<*const std::ffi::c_char> =
      validation_layers.iter().map(|name| name.as_ptr()).collect();
    (validation_layers, vl_pointers)
  };

  #[cfg(feature = "vulkan_vl")]
  let (instance, mut debug_utils) = {
    let debug_create_info = DebugUtils::get_debug_messenger_create_info();
    let instance = instance::create_instance(&entry, &vl_pointers, &debug_create_info);
    let debug_utils = DebugUtils::setup(&entry, &instance, debug_create_info);

    (instance, debug_utils)
  };

  #[cfg(not(feature = "vulkan_vl"))]
  let instance = instance::create_instance(&entry);

  let device_extensions: Vec<String> = DEVICE_EXTENSIONS.iter().map(|x| x.to_string()).collect();
  let (physical_device, queue_family_indices) =
    unsafe { select_physical_device(&instance, &device_extensions) };

  #[cfg(feature = "vulkan_vl")]
  let (logical_device, queues) = create_logical_device(
    &instance,
    &physical_device,
    &device_extensions,
    &queue_family_indices,
    &vl_pointers,
  );
  #[cfg(not(feature = "vulkan_vl"))]
  let (logical_device, queues) = create_logical_device(
    &instance,
    &physical_device,
    &device_extensions,
    &queue_family_indices,
  );

  println!("Successfully created a logical device!");

  // Cleanup
  unsafe {
    log::debug!("Destroying device");
    logical_device.destroy_device(None);

    #[cfg(feature = "vulkan_vl")]
    {
      log::debug!("Destroying debug utils messenger");
      debug_utils.destroy_self();
    }

    log::debug!("Destroying instance");
    instance.destroy_instance(None);
  }
}
