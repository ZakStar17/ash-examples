use std::fmt::Display;

use crate::utility;

// implements some display properties for vendors
pub enum Vendor {
  Nvidia,
  Amd,
  Arm,
  Intel,
  ImgTec,
  Qualcomm,
  Unknown(u32),
}

// support struct for displaying vendor information
impl Vendor {
  pub fn from_id(id: u32) -> Self {
    // some known ids
    match id {
      0x1002 => Self::Amd,
      0x1010 => Self::ImgTec,
      0x10DE => Self::Nvidia,
      0x13B5 => Self::Arm,
      0x5143 => Self::Qualcomm,
      0x8086 => Self::Intel,
      _ => Self::Unknown(id),
    }
  }

  pub fn parse_driver_version(&self, v: u32) -> String {
    // Different vendors can use their own version formats
    // The Vulkan format is (3 bits), major (7 bits), minor (10 bits), patch (12 bits), so vendors
    // with other formats need their own parsing code
    match self {
      Self::Nvidia => {
        // major (10 bits), minor (8 bits), secondary branch (8 bits), tertiary branch (6 bits)
        let eight_bits = 0b11111111;
        let six_bits = 0b111111;
        format!(
          "{}.{}.{}.{}",
          v >> (32 - 10),
          v >> (32 - 10 - 8) & eight_bits,
          v >> (32 - 10 - 8 - 8) & eight_bits,
          v & six_bits
        )
      }
      _ => utility::parse_vulkan_api_version(v),
    }
  }
}

impl Display for Vendor {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Nvidia => f.write_str("NVIDIA"),
      Self::Amd => f.write_str("AMD"),
      Self::Arm => f.write_str("ARM"),
      Self::Intel => f.write_str("INTEL"),
      Self::ImgTec => f.write_str("ImgTec"),
      Self::Qualcomm => f.write_str("Qualcomm"),
      Self::Unknown(id) => f.write_fmt(format_args!("Unknown ({})", id)),
    }
  }
}
