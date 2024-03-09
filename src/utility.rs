use std::ffi::{c_char, CStr, FromBytesUntilNulError};

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

pub fn c_char_array_to_string(arr: &[c_char]) -> String {
  let raw_string = unsafe { CStr::from_ptr(arr.as_ptr()) };
  raw_string
    .to_str()
    .expect("Failed to convert raw string")
    .to_owned()
}

pub unsafe fn i8_array_as_cstr<'a>(arr: &'a [i8]) -> Result<&'a CStr, FromBytesUntilNulError> {
  CStr::from_bytes_until_nul(std::mem::transmute(arr))
}

pub fn error_chain_fmt(
  e: &impl std::error::Error,
  f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
  writeln!(f, "{}\n", e)?;
  let mut current = e.source();
  while let Some(cause) = current {
    writeln!(f, "Caused by:\n\t{}", cause)?;
    current = cause.source();
  }
  Ok(())
}

pub trait OnErr<T, E> {
  fn on_err<O: FnOnce(&E)>(self: Self, op: O) -> Result<T, E>
  where
    Self: Sized;
}

impl<T, E> OnErr<T, E> for Result<T, E> {
  fn on_err<O: FnOnce(&E)>(self, op: O) -> Result<T, E>
  where
    Self: Sized,
  {
    if let Err(ref e) = self {
      op(e);
    }
    self
  }
}

// transmute literals to static CStr
#[macro_export]
macro_rules! cstr {
  ( $s:literal ) => {{
    unsafe { std::mem::transmute::<_, &CStr>(concat!($s, "\0")) }
  }};
}

#[macro_export]
macro_rules! const_flag_bitor {
  ($t:ty, $x:expr, $($y:expr),+) => {
    // ash flags don't implement const bitor
    <$t>::from_raw(
      $x.as_raw() $(| $y.as_raw())+,
    )
  };
}
