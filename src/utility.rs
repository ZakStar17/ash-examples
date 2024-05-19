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

pub unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
  std::slice::from_raw_parts((p as *const T) as *const u8, std::mem::size_of::<T>())
}

pub trait OnErr<T, E> {
  fn on_err<O: FnOnce(&E)>(self, op: O) -> Result<T, E>
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

macro_rules! const_flag_bitor {
  ($t:ty, $x:expr, $($y:expr),+) => {
    // ash flags don't implement const bitor
    <$t>::from_raw(
      $x.as_raw() $(| $y.as_raw())+,
    )
  };
}
pub(crate) use const_flag_bitor;

// populate_array_with_expression!(a + b, 3) transforms into [a + b, a + b, a + b]
#[macro_export]
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
pub use populate_array_with_expression;
