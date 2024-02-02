# Compute shader on a storage image (Mandelbrot)

This example runs a compute shader on a storage image in order to draw the Mandelbrot set.

This is a continuation of the [Image clear example](https://github.com/ZakStar17/ash-by-example/tree/main/compute_image_clear), this time having a compute pipeline, a pipeline cache, descriptor sets and a shader that uses specialization constants.

The shader resides in `src/shaders/shader.glsl`. You can compile this shader by running the `compile_shaders.sh` script.

Even though the program is not iterative, some constants can be changed to generate a different image. These include zoom, coordinates of the center (in the complex plane) as well as maximum number of iterations in the generation algorithm.

You can run this example with:

`RUST_LOG=debug cargo run --bin storage_image_compute_shader`

## Code overview

- This time the device image is created with the `STORAGE` flag and a predefined format used by the compute shader.
- A descriptor set layout is created that describes one storage attachment. This is later used when creating the descriptor pool as well as in the pipeline.
- A descriptor pool is created and one descriptor set is allocated that corresponds to the storage image attachment. An image view is created that describes the full size view with default channels of the local image that is going to be used as storage. This view is written to the descriptor set as well as a corresponding sampler (the sampler is not used as the image is not used as a sampled image, however it is still required in `vk::DescriptorImageInfo`.
- A pipeline cache is created. In order for the driver to not recompile the `.spv` shader, the pipeline cache data is saved and loaded across program invocations.
- The compute shader is loaded and populated with constant values from specialization constants. This shader is used in the compute pipeline creation.
- The compute command buffer binds the storage image descriptor set and dispatches the compute shader. Image barriers and layouts are changed in order to have compatible layouts with the shader and guarantee that the compute operation is completed before transfer.
- All other operations are equal to the previous example. The work is submitted, the image is copied and saved.

The program uses dynamic local groups in the shader, meaning that it can change the size of work groups by passing the value as a specialization constant. However, this requires enabling the `maintenance4` feature.

This example only uses compute and transfer queues.

## Cargo features

This example implements the following cargo features:

- `vl`: Enable validation layers.
- `load`: Load the system Vulkan Library at runtime.
- `link`: Link the system Vulkan Library at compile time.

`vl` and `load` are enabled by default. To disable them, pass `--no-default-features` to cargo.
For example:

`cargo run --release --no-default-features --features link`

For more information about linking and loading check
[https://docs.rs/ash/latest/ash/struct.Entry.html](https://docs.rs/ash/latest/ash/struct.Entry.html).
