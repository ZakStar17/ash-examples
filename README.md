# Compute image clear

This example clears an image to a specific color and saves it to a file. It is the first example that instructs the device to execute a set of commands.

Commands rundown:

- Allocate an image in device (like a GPU) dedicated memory.
- Allocate a buffer in host (like a CPU) accessible memory.
- Clear the image with the help of a Vulkan command.
- Copy the image contents to the buffer.
- Save the buffer contents to a file.

This may seem quite simple, however it already requires pipeline barriers to synchronize operations like image layout transitions and ownership transfer between queues.

You can run this example with:

`RUST_LOG=debug cargo run --bin compute_image_clear`

# Overview

## Command buffers and command pools

Host to device communications are expensive, so Vulkan aids to reduce the amount of communications sent by prerecording all possible commands beforehand and sending them all in a single batch. Regions of memory where commands are recorded are called command buffers. Work batches may consist of multiple command buffers, some of which may be reused between submissions or be executed multiple times simultaneously.

Command buffers have a complicated memory layout, so they are managed by command pools. These hold command buffers of a similar type (for example, ones that are rerecorded constantly) and manage command buffers allocations and destruction. All command buffers belonging to a command pool can only target one specific queue family that was indicated during the creation of said command pool.

In a program execution, a command buffer may be in one of the following states (see https://docs.vulkan.org/spec/latest/chapters/cmdbuffers.html#commandbuffers-lifecycle for more information):

- Initial: All command buffers start in this state and contain no recorded commands. They must be moved to the `Recording` state.
- Recording: Command buffers are set to the recording state with `device.begin_command_buffer()`. Commands like `cmd_clear_color_image` can be recorded.
- Executable: It has finished recording (using `device.end_command_buffer()`) and is ready to be submitted in a work batch.
- Pending: It is currently being executed in the device. Attempting to modify a command buffer in this state is very bad and may lead to the device becoming "lost", in which it is becomes unusable.
- Invalid: Some resource that was recorded was modified or deleted. This command buffer is now invalid but may still be rerecorded and doesn't affect the program as a whole.

Command buffers have the notion of being "reset", in which their state is set to `Initial`. This can be done while the command buffer is in any state that is not `Pending`. Command buffers can be reset individually if a specific flag is set during command pool creation, but more often all command buffers in a command pool will be reset at the same time as it is more optimal.

Command buffers can also be "primary" or "secondary". Secondary command buffers can be recorded as standalone operations inside other primary or secondary command buffers, but can't be submitted. They are useful if you have a set commonly reused operations inside a primary command buffer that is frequently rerecorded. 

## Buffers and Images

You can use two primary objects that hold data which can be accessed by a device, these being images and buffers. Buffers are just an array of arbitrary data, while images are 1, 2 or 3 dimensional collections of pixels that have a specific format, memory layout and can consist of multiple layers.

After a buffer or image is created, its memory should be allocated and bind separately. Memory allocation and management is a manual task in Vulkan but you can use additional libraries to the job for you, like the [AMD Vulkan Memory Allocator (VMA)](https://github.com/gwihlidal/vk-mem-rs). The primary rule is that you should use as few allocations as possible and try to suballocate as much as you can, unless the objects that you are suballocating have different memory requirements or the memory block hit some device limit. This is primarily for performance reasons but also because some systems will have a relatively low device limit on the number of total allocations that you are allowed to perform.

As this example only creates two objects (a buffer and an image) the memory allocation is quite simple, only needing one block for the image and one for the buffer.

## Memory heaps and memory types

Each device will will probably contain different heaps which are physical locations like RAM and GPU local memory. Each of these heaps will contain one or more memory types which are logical locations with specific characteristics. An resource like an image may only reside in specific memory types because of its internal structure, which are queried after the resource is created. You can query the size of a heap but not more than that, so allocations may revolve to some trial and error if you are trying to allocate a very big memory block.

The most important properties of a memory type are:

 - `DEVICE_LOCAL_BIT`: The most efficient for device access.
 - `HOST_VISIBLE_BIT`: Can be mapped by the host, meaning it can be written and read by the host.
 - `HOST_COHERENT_BIT`: Memory writes will always be visible to the host and reads will be always be visible to the device (more explanations bellow).
 - `HOST_CACHED_BIT`: Memory is cached on the host and potentially faster. This is not incompatible with `HOST_COHERENT_BIT`.

See https://docs.vulkan.org/spec/latest/chapters/memory.html#memory-device-properties for more information. There will be always one memory type with the `DEVICE_LOCAL_BIT` and one memory type with the `HOST_VISIBLE_BIT` and `HOST_COHERENT_BIT` flags.

A memory heap will usually have more than one memory type with the same properties. This is because even though objects may reside in the same heap, that may not be the case for one memory type. Querying for memory types will return them in order of increased performance (given the same characteristics), so if you have more than one memory type that is compatible with your objects you can always take the first one.

Memory that is not device local will usually have a lot worse performance because of the speed in which data may be accessed, specially for dedicated cards. Because of this, it will probably be more efficient to write something in a host accessible staging buffer and copy it to a device local buffer than have it be accessed directly, unless the memory is only used a few times.

In this example, the image will be allocated preferably to memory with the `DEVICE_LOCAL_BIT` set, and the buffer to memory with the `HOST_COHERENT_BIT`, and if possible also the `HOST_CACHED_BIT`. Even if a preferred memory type doesn't exist, the code can always fallback to a more general type that can be allocated.

## Vulkan's execution model

GPUs have lots of caches and are complicated, and so is knowing when a command runs or if it has finished. When writing device commands is good to have in mind that:

 - Things may always run at the same time unless is explicitly stated otherwise;
 - Even if a command writes to a block of memory and another command tries to read the same block, it may read different things because that data is residing in different caches that have to otherwise be explicitly flushed;

Making sure that things run in order is called introducing an execution dependency and making sure caches are properly flushed and visible to commands is called an memory dependencies. In order to create these dependencies three types of synchronizations objects can be used:

- Fences: They indicate to the host that a specific set of commands has finished execution.
- Semaphores: They introduce memory and execution dependencies between queues (including queues from different queue families). There are extended semaphores called "timeline semaphores" that can also introduce execution dependencies between the host.
- Pipeline barriers introduce memory and execution barriers in a queue. They are special as they are recorded to command buffers and can perform additional operations like image layout transitions and queue ownership transfers.

Graphics operations can be a bit simpler in terms of execution and memory dependencies as these are more easily managed in a render pass, which will be covered in another example. Even so, creating a render pass and doing anything more complicated than rendering a triangle will always involve some execution and memory barriers that are written in the render pass or declared by pipeline barriers.

You should have a close look at https://docs.vulkan.org/spec/latest/chapters/synchronization.html as this is by far the easiest thing to mess up while writing Vulkan code (even with validation layers).

### Fences

Fences are really simple objects that can be in an signaled or unsignaled state. When submitting a work batch, you can pass one fence that will be signaled once that batch finishes all operations. On the host side, you can reset the fence to an unsignaled state, wait for it to be signaled or simply query this state.

### Pipeline stages

GPUs execute work in stages. These can execute in some order for graphics operations but otherwise occur simultaneously and at any order. Even though pipelines won't be used in this example, other action commands (like copying buffer contents) still execute in one or more specific pipeline stages.

Vulkan used to only have a 32-bit mask of possible stage flags, but with the inclusion of the synchronization2 feature, more stages and a subset of the original ones where introduced in a extended 64-bit mask. For example, Vulkan used to only have the `PIPELINE_STAGE_TRANSFER_BIT` stage that encapsulated all transfer operations, but more concrete stages were introduced like `PIPELINE_STAGE_2_COPY_BIT` for copy operations and `PIPELINE_STAGE_2_CLEAR_BIT` for clear operations.

For example, the `cmd_clear_color_image` is a operation that is only possible in compute or graphics queues and occurs in the `PIPELINE_STAGE_2_CLEAR_BIT` pipeline stage. All of this can be looked in the documentation of the respective command.

Execution dependencies work with pipeline stages, so if you want a clear command to finish before a copy command, you have to indicate the appropriate stages (more on this later).

### Semaphores

This explanation will focus only on binary semaphores, and timeline semaphores will be covered in another example.

Binary semaphores have a signaled and unsignaled stage, similarly to fences. They impose a execution/memory dependency between work submissions in one or more queues. When submitting a work batch, you can indicate a set of semaphores to be waited upon, for which pipeline stages should each of these semaphores wait (and make available) and you can also indicate a set of semaphores to signal.

Each time a binary semaphore is in use, there should be only one operation signaling and one waiting on it.

For example, image you have a semaphore named "all_transfer_finished" and two work batches A and B.

```rust
let all_transfer_finished = create_semaphore(&device)?;
let batch_a_submit_info = vk::SubmitInfo {
    signal_semaphore_count: 1,
    p_signal_semaphores: &all_transfer_finished,
    ...  // doesn't matter
};
let all_transfer_finished_wait_stage = vk::PipelineStageFlags::TRANSFER;
let batch_b_submit_info = vk::SubmitInfo {
    wait_semaphore_count: 1,
    p_wait_semaphores: &image_clear_finished,
    p_wait_dst_stage_mask: &all_transfer_finished_wait_stage,
    ...
};

device.queue_submit(queue_a, &[batch_a_submit_info], vk::Fence::null());  // submit batch A
device.queue_submit(queue_b, &[batch_b_submit_info], vk::Fence::null());  // submit batch B
```

Work batches A and B will execute at the same time as usual, however, because of the semaphore and the dst_stage_mask (the pipeline stage for which the semaphore should wait and flush) that we indicated as TRANSFER, any commands that execute in a transfer pipeline stage in work batch B will only start executing after all transfer commands in work batch A finish, and all memory that was affected by the transfer commands in work batch A will be available to B.

In order words, with this configuration, transfer commands in B will execute after the ones in A, but other commands like running a compute shader in B can execute at the same time as any other command in A, unless another execution dependency is set. The same goes for memory availability.

### Pipeline barriers

Pipeline barriers are command that are recorded to a command buffer. They don't execute in any stage, and instead create execution or memory dependencies between pipeline stages.

The `cmd_pipeline_barrier` is the original command while the `cmd_pipeline_barrier2` is the extended command which is enabled with the synchronization2 feature. Both of these commands work with memory barrier objects. The original command included the execution dependency in the command itself while the extended takes that dependency in each memory barrier object instead. Because execution and memory objects are closely related, meaning that the memory access masks that are allowed are dependent on the pipeline stages, this example will explain pipeline barriers in the context of the extended version, and it is easy to substitute the extended version for the original (just subdivide each `cmd_pipeline_barrier2` into `cmd_pipeline_barrier` commands that have the same execution dependency).

#### Memory barriers

The best way to understand pipeline barriers is with examples. Here is a basic memory barrier:

```rust
// ...clear commands

let copy_after_clear = vk::MemoryBarrier2 {
    src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
    dst_access_mask: vk::AccessFlags2::TRANSFER_READ,
    src_stage_mask: vk::PipelineStageFlags2::CLEAR,
    dst_stage_mask: vk::PipelineStageFlags2::COPY,
    ...
};
// dependency_info() is a helper function that creates a vk::DependencyInfo. See src/command_pools/mod.rs
device.cmd_pipeline_barrier2(command_buffer, &dependency_info(&[], &[], &[prepare_image]));

// ...copy commands
```

`src_stage_mask` and `dst_stage_mask` constitute a execution dependency. They say, "any stage indicated in `dst_stage_mask` should only begin after all stages in `src_stage_mask` finish. In this example, any "copy" command will start executing after all previous "clear" commands finish.

`src_access_mask` and `dst_access_mask` constitute a memory dependency. Values in these masks can be something_WRITE or something_READ. They say:

- If `src_access_mask` is _WRITE and `dst_access_mask` is _READ, then any memory written in any stage in `src_stage_mask` will be made available to read in all stages indicated in `dst_stage_mask`. This corresponds to doing a cache clean (a flush) and prevents read-after-write hazards.
- If `src_access_mask` is _WRITE and `dst_access_mask` is _WRITE, then any memory written in any stage in `src_stage_mask` will be made available to be overwritten in all stages indicated in `dst_stage_mask`. This prevents write-after-write hazards (so that changes made by the first write don't get lost in the cache).

Basically, all combinations of memory in `src_stage_mask` + `src_access_mask` will be made available to all combinations of memory `dst_stage_mask` + `dst_access_mask`. It doesn't make sense to indicate any _READ access in `src_access_mask` as it doesn't perform any memory changes that should be made available. In this example, all memory written by "clear" commands will be visible to any "copy" command when it executes.

This mask barrier only affects commands that belong to the marked src_stages before the pipeline barrier and only affects the commands that belong to dst_stages after the barrier. Any other commands are free to execute in any order unless more dependencies are introduced.


// wrong 
`HOST_READ` and `HOST_WRITE` are a bit special type of `vk::AccessFlags2`. `HOST_READ` makes memory available to the host and `HOST_WRITE` flushes memory written by the host. These are only needed if the memory type that is operated on doesn't have the `HOST_COHERENT_BIT` and  

#### Buffer memory barriers

Buffer memory barriers are very similar to normal memory barriers. The only difference is that they restrict the execution and memory dependencies to just a subsection of a buffer instead of all memory objects.





  They say, "make all memory accessed in the 



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
