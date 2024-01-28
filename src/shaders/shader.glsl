#version 450

layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;

layout(set = 0, binding = 0, rgba8) uniform writeonly image2D output_image;


void main() {
    vec2 image_dimensions = vec2(imageSize(output_image));
    if(gl_GlobalInvocationID.x >= image_dimensions.x || gl_GlobalInvocationID.y >= image_dimensions.y) {
       return;
    }

    float max_iterations = 10000;

    // https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set
    int i;
    float x0 = (float(gl_GlobalInvocationID.x)) / 8000.0 * 2.47 - 2.0;
    float y0 = (float(gl_GlobalInvocationID.y)) / 8000.0 * 2.24 - 1.12;
    vec2 z = vec2(0.0, 0.0);
    for (i = 0; i < max_iterations; i += 1) {
        z = vec2(
            z.x * z.x - z.y * z.y + x0,
            z.y * z.x + z.x * z.y + y0
        );

        if (length(z) > 2.0) {
            break;
        }
    }

    // i is the number of iterations that took to "bail out", and it is normally used to calculate
    // the color
    // You can use any algorithm for this and even use a multicolor pallete (for example by getting
    // the color from a sampled image)
    // Here I am just assigning some predefined values in a monochromatic fashion
    float pallete_val;
    if (i < (max_iterations / 1000)) {
        pallete_val = 0.5;
    } else if (i < (max_iterations / 500)) {
        pallete_val = 0.45;
    } else if (i < (max_iterations / 250)) {
        pallete_val = 0.4;
    } else if (i < (max_iterations / 125)) {
        pallete_val = 0.3;
    } else if (i < max_iterations) {
        pallete_val = 0.2;
    } else {
        pallete_val = 0.0;
    }
    vec4 write_color = vec4(pallete_val, pallete_val, pallete_val, 1.0);

    // all pixels should be assigned a specific color as the starting image is undefined
    imageStore(output_image, ivec2(gl_GlobalInvocationID.xy), write_color);
}