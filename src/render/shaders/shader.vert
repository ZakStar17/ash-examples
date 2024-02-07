#version 450

layout(push_constant) uniform PushConstantData {
  vec2 position;
  float zoom;
} pc;

// vertex
layout(location = 0) in vec2 pos;
layout(location = 1) in vec2 tex_coords;

layout(location = 0) out vec2 out_tex_coords;

void main() {
  gl_Position = vec4(pos * pc.zoom + pc.position, 1.0, 1.0);
  out_tex_coords = tex_coords;
}
