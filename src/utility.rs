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
    if b == 0 {
      break;
    }
    bytes.push(b as u8)
  }
  String::from_utf8(bytes)
}

// returns all values from the iterator not contained in the slice
pub fn not_in_slice<'a, 'b, A: Ord, B, F>(
  available: &'a mut [A],
  required: &mut dyn Iterator<Item = &'b B>,
  cmp: F,
) -> Vec<&'b B>
where
  F: Fn(&'a A, &'b B) -> std::cmp::Ordering,
{
  available.sort();

  let mut unavailable = Vec::new();
  for b in required {
    if available.binary_search_by(|a| cmp(a, b)).is_err() {
      unavailable.push(b);
    }
  }
  unavailable
}

pub fn not_in_string_slice<'a, 'b>(
  available: &'a mut [String],
  required: &mut dyn Iterator<Item = &'b &str>,
) -> Vec<&'b &'b str> {
  not_in_slice(available, required, |av, req| av.as_str().cmp(req))
}
