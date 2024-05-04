use super::vertex::Vertex;

// square
pub const VERTICES: [Vertex; 4] = [
  // top left
  Vertex {
    pos: [0.0, 0.0],
    tex_coords: [0.0, 0.0],
  },
  // bottom left
  Vertex {
    pos: [2.0, 0.0],
    tex_coords: [1.0, 0.0],
  },
  // top right
  Vertex {
    pos: [0.0, 2.0],
    tex_coords: [0.0, 1.0],
  },
  // bottom right
  Vertex {
    pos: [2.0, 2.0],
    tex_coords: [1.0, 1.0],
  },
];
pub const INDICES: [u16; 6] = [0, 1, 2, 3, 2, 1];

// represents a position of the object that will be rendered
#[repr(C)]
pub struct RenderPosition {
  position: [f32; 2],
  ratio: [f32; 2], // width and height in relation to the surface
}

impl RenderPosition {
  pub fn new(position: [f32; 2], ratio: [f32; 2]) -> Self {
    Self { position, ratio }
  }
}
