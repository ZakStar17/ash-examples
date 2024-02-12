use crate::render::Vertex;

const WIDTH: f32 = 28.0;
const HEIGHT: f32 = 44.0;
const OFFSET_X: f32 = 0.0;
const OFFSET_Y: f32 = 0.0;

// square
pub const VERTICES: [Vertex; 4] = [
  // top left
  Vertex {
    pos: [0.0, 0.0],
    tex_coords: [OFFSET_X, OFFSET_Y],
  },
  // top right
  Vertex {
    pos: [2.0, 0.0],
    tex_coords: [OFFSET_X + WIDTH, OFFSET_Y],
  },
  // bottom left
  Vertex {
    pos: [0.0, 2.0],
    tex_coords: [OFFSET_X, OFFSET_Y + HEIGHT],
  },
  // bottom right
  Vertex {
    pos: [2.0, 2.0],
    tex_coords: [OFFSET_X + WIDTH, OFFSET_Y + HEIGHT],
  },
];
pub const INDICES: [u16; 6] = [0, 1, 2, 3, 2, 1];

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
        OFFSET_X + (WIDTH * texture_i as f32),
        OFFSET_Y,
      ],
    }
  }
}
