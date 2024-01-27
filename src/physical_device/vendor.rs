use crate::utility;

// implements some display properties for vendors
pub enum Vendor {
  NVIDIA,
  AMD,
  ARM,
  INTEL,
  ImgTec,
  Qualcomm,
  Unknown(u32),
}

// support struct for displaying vendor information
impl Vendor {
  pub fn from_id(id: u32) -> Self {
    // some known ids
    match id {
      0x1002 => Self::AMD,
      0x1010 => Self::ImgTec,
      0x10DE => Self::NVIDIA,
      0x13B5 => Self::ARM,
      0x5143 => Self::Qualcomm,
      0x8086 => Self::INTEL,
      _ => Self::Unknown(id),
    }
  }

  pub fn parse_driver_version(&self, v: u32) -> String {
    // Different vendors can use their own version formats
    // The Vulkan format is (3 bits), major (7 bits), minor (10 bits), patch (12 bits), so vendors
    // with other formats need their own parsing code
    match self {
      Self::NVIDIA => {
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

impl ToString for Vendor {
  fn to_string(&self) -> String {
    match self {
      Self::NVIDIA => "NVIDIA".to_owned(),
      Self::AMD => "AMD".to_owned(),
      Self::ARM => "ARM".to_owned(),
      Self::INTEL => "INTEL".to_owned(),
      Self::ImgTec => "ImgTec".to_owned(),
      Self::Qualcomm => "Qualcomm".to_owned(),
      Self::Unknown(id) => format!("Unknown ({})", id),
    }
  }
}
