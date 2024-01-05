use std::ffi::{c_char, CStr};

use ash::vk;

pub fn i8_array_to_string(arr: &[i8]) -> Result<String, std::string::FromUtf8Error> {
  let mut bytes = Vec::with_capacity(arr.len());
  for &b in arr {
    if b == 0 {
      break;
    }
    bytes.push(b as u8)
  }
  String::from_utf8(bytes)
}

pub fn c_char_array_to_string(arr: &[c_char]) -> String {
  let raw_string = unsafe { CStr::from_ptr(arr.as_ptr()) };
  raw_string
    .to_str()
    .expect("Failed to convert raw string")
    .to_owned()
}

pub fn parse_vulkan_api_version(v: u32) -> String {
  format!(
    "{}.{}.{}",
    vk::api_version_major(v),
    vk::api_version_minor(v),
    vk::api_version_patch(v)
  )
}

pub fn contains_all<'a, T>(slice: &mut [T], other: &'a [T]) -> Result<(), &'a T>
where
  T: Eq + Ord,
{
  slice.sort();
  for item in other.iter() {
    if let Err(_) = slice.binary_search(item) {
      return Err(item);
    }
  }
  return Ok(());
}
