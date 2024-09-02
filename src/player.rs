use crate::{render::SpritePushConstants, PressedKeys};

pub struct Player {
  pub position: [f32; 2],
}

impl Player {
  // pixels per second
  pub const SPEED_HOR: f32 = 1.0;
  pub const SPEED_VER: f32 = 1.0;

  //    -1.0
  // -1.0 +------> 1.0
  //      |
  //      |
  //      v
  //     1.0
  pub fn new(position: [f32; 2]) -> Self {
    Self { position }
  }

  pub fn update(&mut self, delta_secs: f32, keys: &PressedKeys) {
    let hor = if keys.left {
      if !keys.right {
        -Self::SPEED_HOR
      } else {
        0.0
      }
    } else if keys.right {
      Self::SPEED_HOR
    } else {
      0.0
    };

    let ver = if keys.up {
      if !keys.down {
        -Self::SPEED_VER
      } else {
        0.0
      }
    } else if keys.down {
      Self::SPEED_VER
    } else {
      0.0
    };

    self.position[0] += hor * delta_secs;
    self.position[1] += ver * delta_secs;
  }

  fn texture_index(keys: &PressedKeys) -> usize {
    if keys.left && !keys.right {
      2
    } else if keys.right && !keys.left {
      1
    } else {
      0
    }
  }

  pub fn sprite_data(&self, keys: &PressedKeys) -> SpritePushConstants {
    SpritePushConstants::new(self.position, Self::texture_index(keys))
  }
}
