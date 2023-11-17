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
