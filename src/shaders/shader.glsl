#version 450

layout (constant_id = 2) const int MAX_ITERATIONS = 10000;

// coordinates of the image center
layout (constant_id = 3) const float FOCAL_POINT_X = -0.765;
layout (constant_id = 4) const float FOCAL_POINT_Y = 0.0;
layout (constant_id = 5) const float ZOOM = 1.0;

// uses index 0 specialization constant as the local group size for x and y
layout(local_size_x_id = 0, local_size_y_id = 1, local_size_z = 1) in;

layout(set = 0, binding = 0, rgba8) uniform writeonly image2D output_image;


void main() {
    vec2 img_size = vec2(imageSize(output_image));
    if(gl_GlobalInvocationID.x >= img_size.x || gl_GlobalInvocationID.y >= img_size.y) {
        // return early if outside of image
        return;
    }

    // normalize and correct for aspect ratio
    float norm_x = (float(gl_GlobalInvocationID.x) - (img_size.x / 2.0)) / img_size.x;
    float norm_y = (float(gl_GlobalInvocationID.y) - (img_size.y / 2.0)) / img_size.x;

    float x0 = (norm_x / ZOOM) + FOCAL_POINT_X;
    float y0 = (norm_y / ZOOM) + FOCAL_POINT_Y;

    // https://en.wikipedia.org/wiki/Plotting_algorithms_for_the_Mandelbrot_set
    int i;
    vec2 z = vec2(0.0, 0.0);
    for (i = 0; i < MAX_ITERATIONS; i += 1) {
        z = vec2(
            z.x * z.x - z.y * z.y + x0,
            z.y * z.x + z.x * z.y + y0
        );

        if (length(z) > 2.0) {
            break;
        }
    }

    // i is the number of iterations that took to "bail out", and can be used to calculate the color
    // You can use any algorithm for this and even use a multicolor pallete (for example by getting
    // the color from a sampled image)
    // Here I am just assigning some predefined values in a monochromatic fashion
    float pallete_val;
    if (i < (MAX_ITERATIONS / 1000)) {
        pallete_val = 0.5;
    } else if (i < (MAX_ITERATIONS / 500)) {
        pallete_val = 0.45;
    } else if (i < (MAX_ITERATIONS / 250)) {
        pallete_val = 0.4;
    } else if (i < (MAX_ITERATIONS / 125)) {
        pallete_val = 0.3;
    } else if (i < MAX_ITERATIONS) {
        pallete_val = 0.2;
    } else {
        pallete_val = 0.0;
    }
    vec4 write_color = vec4(pallete_val, pallete_val, pallete_val, 1.0);

    // all pixels should be assigned a specific color as the starting image is undefined
    imageStore(output_image, ivec2(gl_GlobalInvocationID.xy), write_color);
}