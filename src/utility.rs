use std::ffi::{c_char, CStr};

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

pub fn i8_array_to_string(arr: &[i8]) -> Result<String, std::string::FromUtf8Error> {
  let mut bytes = Vec::with_capacity(arr.len());
  for &b in arr {
    if b == '\0' as i8 {
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

// returns all values from the iterator not contained in the slice
pub fn not_in_slice<'a, 'b, A: Ord, B: ?Sized, F>(
  slice: &'a mut [A],
  iter: &mut dyn Iterator<Item = &'b B>,
  f: F, // comparison function between items in slice and iter
) -> Box<[&'b B]>
where
  F: Fn(&'a A, &'b B) -> std::cmp::Ordering,
{
  slice.sort();
  iter
    .filter(|b| slice.binary_search_by(|a| f(a, b)).is_err())
    .collect()
}

// returns all values from the iterator contained in the slice
pub fn in_slice<'a, 'b, A: Ord, B: ?Sized, F>(
  slice: &'a mut [A],
  iter: &mut dyn Iterator<Item = &'b B>,
  f: F, // comparison function between items in slice and iter
) -> Box<[&'b B]>
where
  F: Fn(&'a A, &'b B) -> std::cmp::Ordering,
{
  slice.sort();
  iter
    .filter(|b| slice.binary_search_by(|a| f(a, b)).is_ok())
    .collect()
}

pub unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
  std::slice::from_raw_parts((p as *const T) as *const u8, std::mem::size_of::<T>())
}

// bitor between flags to be used as constants
macro_rules! const_flag_bitor {
  ($t:ty => $x:expr, $($y:expr),+) => {
    <$t>::from_raw(
      $x.as_raw() $(| $y.as_raw())+,
    )
  };
}
pub(crate) use const_flag_bitor;

// transmutes literals to 'static CStr
macro_rules! cstr {
  ( $s:literal ) => {{
    unsafe { std::mem::transmute::<_, &std::ffi::CStr>(concat!($s, "\0")) }
  }};
}
pub(crate) use cstr;

// populate_array_with_expression!(a + b, 3) transforms into [a + b, a + b, a + b]
macro_rules! populate_array_with_expression {
  ($ex:expr, $arr_size:expr) => {{
    use std::mem::MaybeUninit;
    let mut tmp: [MaybeUninit<_>; $arr_size] = unsafe { MaybeUninit::uninit().assume_init() };
    for i in 0..$arr_size {
      tmp[i] = MaybeUninit::new($ex);
    }
    unsafe { std::mem::transmute::<_, [_; $arr_size]>(tmp) }
  }};
}
pub(crate) use populate_array_with_expression;
