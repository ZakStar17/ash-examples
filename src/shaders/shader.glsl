#version 450

layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;

layout(set = 0, binding = 0, rgba8) uniform writeonly image2D output_image;


void main() {
    vec2 image_dimensions = vec2(imageSize(output_image));
    if(gl_GlobalInvocationID.x >= image_dimensions.x || gl_GlobalInvocationID.y >= image_dimensions.y) {
       return;
    }

    vec4 write_color = vec4(0.0, 0.0, 1.0, 1.0);
    imageStore(output_image, ivec2(gl_GlobalInvocationID.xy), write_color);
}