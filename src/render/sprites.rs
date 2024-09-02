use super::Vertex;

pub const SQUARE_INDICES: [u16; 6] = [1, 2, 3, 2, 1, 0];

// square vertices
pub const PLAYER_WIDTH: f32 = 28.0;
pub const PLAYER_HEIGHT: f32 = 44.0;
pub const PLAYER_OFFSET_X: f32 = 0.0;
pub const PLAYER_OFFSET_Y: f32 = 0.0;
pub const PLAYER_VERTICES: [Vertex; 4] = [
  // top left
  Vertex {
    pos: [0.0, 0.0],
    tex_coords: [PLAYER_OFFSET_X, PLAYER_OFFSET_Y],
  },
  // top right
  Vertex {
    pos: [2.0, 0.0],
    tex_coords: [PLAYER_OFFSET_X + PLAYER_WIDTH, PLAYER_OFFSET_Y],
  },
  // bottom left
  Vertex {
    pos: [0.0, 2.0],
    tex_coords: [PLAYER_OFFSET_X, PLAYER_OFFSET_Y + PLAYER_HEIGHT],
  },
  // bottom right
  Vertex {
    pos: [2.0, 2.0],
    tex_coords: [
      PLAYER_OFFSET_X + PLAYER_WIDTH,
      PLAYER_OFFSET_Y + PLAYER_HEIGHT,
    ],
  },
];

const PROJECTILE_WIDTH: f32 = 15.0;
const PROJECTILE_HEIGHT: f32 = 15.0;
const PROJECTILE_OFFSET_X: f32 = 85.0;
const PROJECTILE_OFFSET_Y: f32 = 0.0;
pub const PROJECTILE_VERTICES: [Vertex; 4] = [
  // top left
  Vertex {
    pos: [0.0, 0.0],
    tex_coords: [PROJECTILE_OFFSET_X, PROJECTILE_OFFSET_Y],
  },
  // top right
  Vertex {
    pos: [2.0, 0.0],
    tex_coords: [PROJECTILE_OFFSET_X + PROJECTILE_WIDTH, PROJECTILE_OFFSET_Y],
  },
  // bottom left
  Vertex {
    pos: [0.0, 2.0],
    tex_coords: [PROJECTILE_OFFSET_X, PROJECTILE_OFFSET_Y + PROJECTILE_HEIGHT],
  },
  // bottom right
  Vertex {
    pos: [2.0, 2.0],
    tex_coords: [
      PROJECTILE_OFFSET_X + PROJECTILE_WIDTH,
      PROJECTILE_OFFSET_Y + PROJECTILE_HEIGHT,
    ],
  },
];
