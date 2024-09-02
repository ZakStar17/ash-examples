use super::vertices::Vertex;

// todo: all vertices probably will need to be in the same buffer

type QuadIndex = u16;
pub const QUAD_INDICES: [QuadIndex; 6] = [1, 2, 3, 2, 1, 0];
pub const QUAD_INDEX_COUNT: u32 = QUAD_INDICES.len() as u32;
pub const QUAD_INDICES_SIZE: u64 = (size_of::<QuadIndex>() * QUAD_INDEX_COUNT as usize) as u64;

pub const QUAD_VERTEX_COUNT: usize = 4;
pub const QUAD_VERTICES_SIZE: u64 = (size_of::<Vertex>() * QUAD_VERTEX_COUNT) as u64;

// square vertices
pub const PLAYER_WIDTH: f32 = 28.0;
pub const PLAYER_HEIGHT: f32 = 44.0;
pub const PLAYER_OFFSET_X: f32 = 0.0;
pub const PLAYER_OFFSET_Y: f32 = 0.0;
pub const PLAYER_VERTICES: [Vertex; QUAD_VERTEX_COUNT] = [
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
pub const PROJECTILE_VERTICES: [Vertex; QUAD_VERTEX_COUNT] = [
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
