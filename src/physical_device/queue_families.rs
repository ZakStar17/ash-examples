#[derive(Debug)]
pub struct QueueFamily {
  pub index: u32,
  pub queue_count: u32,
}

// Specialized compute and transfer queue families may not be available
// If so, they will be substituted by the graphics queue family, as a queue family that supports
//    graphics implicitly also supports compute and transfer operations
#[derive(Debug)]
pub struct QueueFamilies {
  pub graphics: QueueFamily,
  pub compute: Option<QueueFamily>,
  pub transfer: Option<QueueFamily>,
  pub unique_indices: Box<[u32]>,
}

impl QueueFamilies {
  pub fn get_compute_index(&self) -> u32 {
    match self.compute.as_ref() {
      Some(family) => family.index,
      None => self.graphics.index,
    }
  }

  pub fn get_transfer_index(&self) -> u32 {
    match self.transfer.as_ref() {
      Some(family) => family.index,
      None => self.graphics.index,
    }
  }
}
