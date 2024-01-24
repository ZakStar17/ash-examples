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
