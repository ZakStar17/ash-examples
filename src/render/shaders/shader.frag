#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 out_color;

void main() {
  out_color = vec4(1.0, tex_coords, 1.0);
}
