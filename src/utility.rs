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

// power is the actual power of 2 (1, 2, 4, 8, 16...)
#[inline]
pub fn round_down_to_power_of_2_u64(n: u64, power: u64) -> u64 {
  // strip right-most <less than power> bits from n
  let rem_error = n & (power - 1);
  n - rem_error
}

// power is the actual power of 2 (1, 2, 4, 8, 16...)
// basically same as rounded_down_to_power_of_2 but
//    adds power to rounded_down_to_power_of_2(n) if n does not already divide power
#[inline]
pub fn round_up_to_power_of_2_u64(n: u64, power: u64) -> u64 {
  let rem_error = n & (power - 1);
  if rem_error > 0 {
    n - rem_error + power
  } else {
    n
  }
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
  ($t:ty => $x:expr, $($y:expr),+) => {
    // ash flags don't implement const bitor
    <$t>::from_raw(
      $x.as_raw() $(| $y.as_raw())+,
    )
  };
}
pub(crate) use const_flag_bitor;

// kinda stolen from https://stackoverflow.com/questions/77027517/how-can-i-perform-compile-time-concatenation-of-array-literals
// copies values from an array of arrays into a flattened single array
pub const fn concatenate_arrays<const N: usize, T: Copy>(array_slice: &[&[T]]) -> [T; N] {
  let mut result: [T; N] = [array_slice[0][0]; N];

  let mut i = 0;
  let mut result_i = 0;
  while i < array_slice.len() {
    let mut j = 0;
    while j < array_slice[i].len() {
      result[result_i] = array_slice[i][j];
      result_i += 1;
      j += 1;
    }
    i += 1;
  }

  result
}

// populate_array_with_expression!(a + b, 3) transforms into [a + b, a + b, a + b]
macro_rules! fill_array_with_expression {
  ($ex:expr, $arr_size:expr) => {{
    use std::mem::MaybeUninit;
    let mut tmp: [MaybeUninit<_>; $arr_size] = unsafe { MaybeUninit::uninit().assume_init() };
    for i in 0..$arr_size {
      tmp[i] = MaybeUninit::new($ex);
    }
    unsafe { std::mem::transmute::<_, [_; $arr_size]>(tmp) }
  }};
}
pub(crate) use fill_array_with_expression;
