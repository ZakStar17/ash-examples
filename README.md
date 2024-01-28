# Compute image clear

This example instructs the GPU to clear a local image to a specific color in device memory, copy it to CPU accessible memory, read it and save it to a file.

You can run this example with:

`RUST_LOG=debug cargo run --bin compute_image_clear`

## Code overview

- The physical device selection contains additional checks for if the requested image format (here `IMAGE_FORMAT = vk::Format::R8G8B8A8_UINT`) is supported and the device allows image creation of the requested size (`IMAGE_WIDTH` and `IMAGE_HEIGHT`).
- Two images are created, one which resides in `DEVICE_LOCAL` memory (local to the GPU) and the other which resides in `HOST_VISIBLE` memory (accessible by the CPU). The contents of the first image will be cleared and then copied to the second one. The host image uses linear tiling in order for its memory to not be implementation dependent.
- Two command buffer pools are created, one for the compute queue family and the other for the transfer. Both command pools allocate one command buffer, the compute is recorded to clear the local image and the transfer to copy it to the host image.
- Both command buffers use image memory barriers to perform layout transitions (these are set to be optimal for their respective use cases). Because each image is created with `vk::SharingMode::EXCLUSIVE`, the pipeline barriers also perform queue family ownership release and acquire.
- The command buffers are submitted and synchronized with an semaphore. The second submit also signals a fence for when work is completed.
- When work finishes, the host image memory is then invalidated, read and saved to an file using `image::save_buffer` from the [image](https://docs.rs/image/latest/image/) crate.

This example does not create any pipelines and only necessitates compute and transfer queues.

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
