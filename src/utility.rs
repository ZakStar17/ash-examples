use std::ffi::{CStr, FromBytesUntilNulError};

use ash::vk;

// this module contains general functions used in other modules

pub fn parse_vulkan_api_version(v: u32) -> String {
  format!(
    "{}.{}.{}",
    vk::api_version_major(v),
    vk::api_version_minor(v),
    vk::api_version_patch(v)
  )
}

pub unsafe fn i8_array_as_cstr(arr: &[i8]) -> Result<&CStr, FromBytesUntilNulError> {
  CStr::from_bytes_until_nul(std::mem::transmute::<&[i8], &[u8]>(arr))
}
