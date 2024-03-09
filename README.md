# Compute image clear

This example instructs the GPU to clear an image with a specific color with the help of a compute queue, then copy the image contents to a buffer with the help of a transfer queue. The buffer memory is later read and saved to a file.

You can run this example with:

`RUST_LOG=debug cargo run --bin compute_image_clear`

## Application overview

The application can be resumed by the following steps:

- Perform initialization: Create an instance, select the physical device, create a logical device, allocate command buffers (one for transfer and one for compute), create an image and a buffer and allocate memory for them.
- Record all the necessary commands to the command buffers. This involves setting appropriate execution and memory barriers.
- Submit the recorded work and wait for it to complete.
- Map buffer contents, access and save them to a file. This is performed with the help of the [image crate](https://docs.rs/image/latest/image/).

## Initialization

### Device selection

New checks were added to check if the device supports the required image format and dimensions. In a more complete application it would be more common to fallback to using other formats and maybe subdivide the target image until it is supported.

### Image and buffer creation

The image and buffer are created with sharing mode set to `vk::SharingMode::EXCLUSIVE`, meaning their ownership has to be managed by the command buffers.

The two objects are allocated separately and as a whole. The created image will have its memory preferably allocated with device local flags, while the buffer's memory will always have the `HOST_VISIBLE` flag as it is necessary for the CPU to access the underlying memory.

### Command buffer pools

The command buffers are each allocated in a separate pool, as they use different queues.

## Command buffer recording

The compute command buffer will execute first and will be immediately followed by the transfer command buffer. 

Because the operations in this example have to execute linearly, there has to exist an execution barrier in order for commands to not run at the same time and adequate memory barriers to make sure written memory from a command is visible to a subsequent command.

Compute commands:

 - Execute an image memory barrier, changing the image layout to `vk::ImageLayout::TRANSFER_DST_OPTIMAL`.
 - Execute `cmd_clear_color_image`.
 - Execute another image memory barrier, performing a ownership transfer release on the image (from the compute to the transfer queue) and at the same time changing its layout to `vk::ImageLayout::TRANSFER_SRC_OPTIMAL`.

Transfer commands:
 - Execute an image memory barrier, acquiring the image with its contents intact from the compute queue. This operation has to match the release barrier from the compute command buffer, meaning it also has to contain the image layout change.
 - Execute `cmd_copy_image_to_buffer`.
 - Execute an image memory barrier to flush the resulting buffer contents to the host, so that they can be accessed later.

This example uses Vulkan 1.3 as well as the synchronization 2 feature, so each barrier object contains a execution scope (src/dst stage) as well as a memory scope (src/dst access), and enables the use of more synchronization flags.

## Work submission

Two submissions are performed, one for transfer and one for compute. Because these have operations that need to be synchronized between the two queues, a (binary) semaphore is created and used in order to guarantee execution order.

A fence is created and made to wait upon the second submission, which once finishes allows reading data from the buffer.

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
