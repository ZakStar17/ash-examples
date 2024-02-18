use super::sprites::{PLAYER_OFFSET_X, PLAYER_OFFSET_Y, PLAYER_WIDTH};

// represents a position of the object that will be rendered
#[repr(C)]
pub struct SpritePushConstants {
  pub position: [f32; 2],
  pub texture_offset: [f32; 2],
}

impl SpritePushConstants {
  pub fn new(position: [f32; 2], texture_i: usize) -> Self {
    Self {
      position,
      // sprite textures are layed out horizontally in the texture file
      texture_offset: [
        PLAYER_OFFSET_X + (PLAYER_WIDTH * texture_i as f32),
        PLAYER_OFFSET_Y,
      ],
    }
  }
}

#[repr(C)]
pub struct ComputePushConstants {
  pub position: [f32; 2],
}
