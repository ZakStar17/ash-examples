#version 450

// vertex
layout(location = 0) in vec2 vertex_pos;
layout(location = 1) in vec2 tex_coords;

// instance
layout(location = 2) in vec2 instance_pos;
layout(location = 3) in vec2 _instance_vel;

layout(location = 0) out vec2 out_tex_coords;

void main() {
  float x = vertex_pos.x * 0.04 + instance_pos.x;
  float y = vertex_pos.y * 0.04 + instance_pos.y;
  gl_Position = vec4(x, y, 1.0, 1.0);
  
  out_tex_coords = tex_coords;
}
