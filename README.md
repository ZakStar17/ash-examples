# Compute image clear

This example clears an image to a specific color and saves it to a file. It is the first example that instructs the device to execute a set of commands.

Commands rundown:

- Allocate an image in device (like a GPU) dedicated memory.
- Allocate a buffer in host (like a CPU) accessible memory.
- Clear the image with the help of a Vulkan command.
- Copy the image contents to the buffer.
- Save the buffer contents to a file.

This may seem quite simple, however it requires knowledge about memory and command synchronization in Vulkan and setting operations like image layout transitions and ownership transfer between queues.

After the image contents are acquired in RAM, the file saving process is done using the [image crate](https://docs.rs/image/latest/image/), as this operation is not important to this example.

You can run this example with:

`cargo run`

# Theory overview

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

After a buffer or image is created, its memory should be allocated and bind separately. Memory allocation and management is a manual task in Vulkan but you can use additional libraries to the job for you, like the [AMD Vulkan Memory Allocator (VMA)](https://github.com/gwihlidal/vk-mem-rs). It is better to use as few allocations as possible and try to suballocate as much as you can, unless the objects that you are suballocating have different memory requirements or the memory block hit some device limit. This is primarily for performance reasons but also because some systems will have a relatively low device limit on the number of total allocations that you are allowed to perform.

As this example only creates two objects (a buffer and an image) the memory allocation is quite simple, only needing one block for the image and one for the buffer.

### Image layouts

Images are supported in opaque memory layouts defined by the implementation (driver). A command recorded in a command buffer can only operate on some specific set of image layouts and so an image layout may have to be transitioned multiple times in a command buffer.

Images can be created in the `UNDEFINED` (undefined contents) or `PREINITIALIZED` (pre-written by the host) layouts, and after that the layout will have to be transitioned to one that is optimal and can be operated by the next command.

There exists a `GENERAL` layout in which image contents are layed out linearly (row by row for 2D images). Most commands can operate on this layout, however won't be as efficient as transitioning the layout to a more optimal and using that one. In case the image contents are to be read by the host, it is generally better to copy the image contents to a buffer and have the host access the buffer instead.

### Image formats

Formats define what information is stored in each texel. Only some formats may be supported in an implementation and some must be enabled with extensions.

This example uses a common image format, namely `R8G8B8A8_UNORM`, which stores texel data in RGBA format with an normalized 8 bit value for each channel. Even so, it is good to have device selection check if this format is supported with the possible image usages and dimensions. In a case the format is not supported, the physical device is skipped in the selection process, but in a more complete application it would be more common to fallback to using other formats.

## Memory heaps and memory types

Each device will will probably contain different heaps which are physical locations like RAM and GPU local memory. Each of these heaps will contain one or more memory types which are logical locations with specific characteristics. An resource like an image may only reside in specific memory types because of its internal structure, which are queried after the resource is created. You can query the size of a heap but not more than that, so allocations may revolve to some trial and error if you are trying to allocate a very big memory block.

The most important properties of a memory type are:

- `DEVICE_LOCAL_BIT`: The most efficient for device access.
- `HOST_VISIBLE_BIT`: Can be mapped by the host, meaning it can be written and read by the host.
- `HOST_COHERENT_BIT`: Memory writes will always be visible to the host and reads will be always be visible to the device (more explanations bellow).
- `HOST_CACHED_BIT`: Memory is cached on the host and potentially faster. This is not incompatible with `HOST_COHERENT_BIT`.

See https://docs.vulkan.org/spec/latest/chapters/memory.html#memory-device-properties for more information. There will be always one memory type with the `DEVICE_LOCAL_BIT` and one memory type with the `HOST_VISIBLE_BIT` and `HOST_COHERENT_BIT` flags.

Created objects and images may only reside in a subset of all memory types depending on which attributes they are created with. Memory heaps may contain multiple different memory types with the same property flags, but each memory type may only support a specific set of images, for example. Even so, because memory types are generally ordered by performance, choosing a compatible memory type usually requires choosing the first one that is compatible with the object for which memory is being allocated.

Memory that is not device local will usually have a lot worse performance because of the speed in which data may be accessed, specially for dedicated cards. Because of this, it will probably be more efficient to write something in a host accessible staging buffer and copy it to a device local buffer than have it be accessed directly, unless the memory is only used a few times.

In this example, the image will be allocated preferably to memory with the `DEVICE_LOCAL_BIT` set, and the buffer to memory with the `HOST_VISIBLE`, and if possible also the `HOST_CACHED_BIT`. Even if a preferred memory type doesn't exist, the code can always fallback to a more general type that can be allocated.

## Vulkan's execution model

Because of GPUs caching and parallelism, having operations follow a specific order and prevent data races is quite complicated. It is good to have in mind that:

- Things may always run at the same time unless explicitly stated otherwise, implicitly (for operations that don't make sense to run at the same time) or explicitly (through synchronization objects);
- Data may reside in different caches which will have to be explicitly flushed in order to avoid data race situations and other memory conflicts, like reading from a cache that has become invalid.

Synchronization objects can be used to introduce execution dependencies to make sure things run in order and memory dependencies to make sure memory is properly visible (meaning that caches are properly flushed). Some of these objects are:

- Fences: They indicate to the host that a specific set of commands has finished execution.
- Semaphores: They introduce memory and execution dependencies between queues (including queues from different queue families). There are extended semaphores called "timeline semaphores" that can also introduce execution dependencies between the host.
- Pipeline barriers introduce memory and execution barriers in a queue. They are recorded to command buffers and can perform additional operations like image layout transitions and queue ownership transfers.

It is important to know well how these work (https://docs.vulkan.org/spec/latest/chapters/synchronization.html) as synchronization is very easy to mess up. Validation layers will usually catch errors like data races however these can be hard to fix without knowing for certain in which order do operations occur.

### Fences

Fences are really simple objects that can be in an signaled or unsignaled state. When submitting a work batch, you can pass one fence that will be signaled once that batch finishes all operations. On the host side, you can reset the fence to an unsignaled state, wait for it to be signaled or simply query this state.

### Pipeline stages

GPUs execute work in stages. These can execute in some order for some operations (like during the different stages of a graphics pipeline) but otherwise occur simultaneously and at any order. Even though pipelines won't be used in this example, other action commands (like copying buffer contents) still execute in one or more specific pipeline stages.

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
    ..vk::MemoryBarrier2::default(),
};
// dependency_info() is a helper function that creates a vk::DependencyInfo. See src/command_pools/mod.rs
device.cmd_pipeline_barrier2(command_buffer, &dependency_info(&[], &[], &[prepare_image]));

// ...copy commands
```

`src_stage_mask` and `dst_stage_mask` constitute a execution dependency. They say, "any stage indicated in `dst_stage_mask` should only begin after all stages in `src_stage_mask` finish. In this example, any "copy" command will start executing after all previous "clear" commands finish.

`src_access_mask` and `dst_access_mask` constitute a memory dependency. Values in these masks can be something_WRITE or something_READ. They say:

- If `src_access_mask` is `_WRITE` and `dst_access_mask` is `_READ`, then any memory written in any stage in `src_stage_mask` will be made available to read in all stages indicated in `dst_stage_mask`. This corresponds to doing a cache clean (a flush) and prevents read-after-write hazards.
- If `src_access_mask` is `_WRITE` and `dst_access_mask` is `_WRITE`, then any memory written in any stage in `src_stage_mask` will be made available to be overwritten in all stages indicated in `dst_stage_mask`. This prevents write-after-write hazards (so that changes made by the first write don't get lost in the cache).

Basically, all combinations of memory in `src_stage_mask` + `src_access_mask` will be made available to all combinations of memory `dst_stage_mask` + `dst_access_mask`. It doesn't make sense to indicate any `_READ` access in `src_access_mask` as it doesn't perform any memory changes that should be made available. In this example, all memory written by "clear" commands will be visible to any "copy" command when it executes.

This mask barrier only affects commands that belong to the marked src_stages before the pipeline barrier and only affects the commands that belong to dst_stages after the barrier, unless the other commands also depend on the commands being affected. For example, if there exists a barrier that indicates that A must happen before B, introducing another barrier which states that B must happen before C will also imply that A must happen before C, creating an execution chain.

#### Buffer and image memory barriers

Buffer and image memory barriers work like normal barriers but restrict the memory dependency to only a region of a buffer or a subresource (aspects and layers) of an image.

These memory barriers can also perform [queue ownership transfers](https://docs.vulkan.org/spec/latest/chapters/synchronization.html#synchronization-queue-transfers) (see bellow) and in case of images can also do [image layout](https://docs.vulkan.org/spec/latest/chapters/resources.html#resources-image-layouts) transitions.

Here is an example of an image barrier that also performs a layout transition (which is also used in this example):

```rust
// select only the color aspect of an image with one single layer and no mipmap levels
let subresource_range = vk::ImageSubresourceRange {
    aspect_mask: vk::ImageAspectFlags::COLOR,
    base_mip_level: 0,
    level_count: 1,
    base_array_layer: 0,
    layer_count: 1,
};
let prepare_image = vk::ImageMemoryBarrier2 {
    src_access_mask: vk::AccessFlags2::NONE,
    dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
    src_stage_mask: vk::PipelineStageFlags2::NONE,
    dst_stage_mask: vk::PipelineStageFlags2::CLEAR,
    old_layout: vk::ImageLayout::UNDEFINED,
    new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
    dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
    image,
    subresource_range,
    ..vk::ImageMemoryBarrier2::default(),
};
device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[prepare_image]));

// clear commands can now properly use the TRANSFER_DST_OPTIMAL layout
```

This image memory barrier is equivalent to this memory barrier:

```rust
let non_image_specific_barrier = vk::MemoryBarrier2 {
    src_access_mask: vk::AccessFlags2::NONE,
    dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
    src_stage_mask: vk::PipelineStageFlags2::NONE,
    dst_stage_mask: vk::PipelineStageFlags2::CLEAR,
    ..vk::MemoryBarrier2::default(),
};
```

However, it only affects the image and also performs an layout transition from `UNDEFINED` to `TRANSFER_DST_OPTIMAL`. Having `src_stage_mask` be `NONE` doesn't create any dependency between commands that may happen before (which would be useless in a standard memory barrier) but, because this barrier also performs an layout transition, it makes sense to have it so that the transition completes before any "clear" commands and so that the new layout is properly made available.

If a `vk::ImageMemoryBarrier2` is used without a image layout transfer, `old_layout` and `new_layout` must be equal and set to the correct current image layout.

`src_queue_family_index` and `dst_queue_family_index` are used for queue ownership transfers and should be set to `vk::QUEUE_FAMILY_IGNORED` if no ownership transfer operation is performed.

## Queue submission memory guarantees and host/device memory synchronization

Memory between host and device may have to be synchronized two times, once as a [Domain operation](https://docs.vulkan.org/spec/latest/appendices/memorymodel.html#memory-model-vulkan-availability-visibility) (to make memory visible to the device or host) and once as a standard memory dependency (to make memory available to commands).

When communicating between host and device, data may reside in the host domain or device domain. Unless the worked memory type is marked with the `HOST_COHERENT_BIT` flag, a explicit memory domain operation must be performed to synchronize data across domains (mapping and unmapping memory does not accomplish this):

- `device.flush_mapped_memory_ranges(ranges)`: Makes all writes in the host domain available to the device domain.
- `device.invalidate_mapped_memory_ranges(ranges)`: Makes all writes in the device domain (that have been made accessible) visible to the host domain.

To actually make host memory that is already in the device's domain available to commands or make command memory writes again visible to the device's domain (note that this doesn't have anything to do with where the object resides or if the memory type has the `HOST_COHERENT_BIT` flag) special access flags `HOST_READ` and `HOST_WRITE` are used. For example:

- A memory dependency with `HOST_WRITE` as `src_access_mask` and `TRANSFER_READ` as `dst_access_mask` will make host writes available to be read by transfer commands.
- A memory dependency with `SHADER_WRITE` as `src_access_mask` and `HOST_READ` as `dst_access_mask` will make shader writes available to the device's (general) domain.

Submitting a batch in a queue submission will perform an implicit memory domain operation from host to device as well as an implicit visibility operation on all host writes. This means that `device.flush_mapped_memory_ranges(ranges)` and a memory dependency with `HOST_WRITE` as `src_access_mask` and `MEMORY_READ` as `dst_access_mask` are performed automatically when submitting work to a queue on all objects that belong to the submitted device. This means that flushing memory and making memory available from the `HOST_WRITE` access flag is not necessary to make the device be able to read memory properly.

In a nutshell, explicit host-device synchronization is needed when:

- A batch has been submitted, and while it is in execution, new writes performed by the host must be made visible and available to the device.
- The host must access data written by the device.

This example records an execution dependency (and if necessary also uses `device.invalidate_mapped_memory_ranges(ranges)`) so that the CPU can read data that has been written by the device and save it to a file.

## Queue ownership transfers

When objects are created without `vk::SharingMode::EXCLUSIVE`, reading data from a buffer range (a section of a buffer) or an image across different queue families returns undefined contents unless memory ownership is properly transferred.

Buffers and images can have different memory layouts across queue family ownerships. Because of that, if for say that you have one section of a buffer, its contents can only be read properly by queues belonging to one queue family at a time, which is said to own the backing memory.

There is no point of transferring ownerships if the memory contents that you are working with can be discarded or be undefined, and performing queue ownerships transfers in this case is harmful. For example, say there is a case where a queue that only performs compute operations writes to an image and later the image is transferred to a queue that performs graphics operations. If the image has to be written again by the compute queue, there shouldn't be any ownership transfer between the graphics queue back to the compute queue unless, of course, the compute queue needs to read the image contents again after they been through the graphics queue.

Ownership transfer operations occur in two distinct pairs:

- An release operation from the source queue family;
- An acquire operation from the destination queue family.

An acquire should always occur after an release in correct order and not at the same time. To accomplish this, it is easy to use a semaphore to define an dependency across the two queues.

Both of the release and acquire operations are defined using pipeline barriers. The release pipeline barrier is written to the command buffer of the queue that first owns the object of which ownership is being transferred and the acquire pipeline barrier is written to the command buffer belonging to the target queue family.

For the next example, let FAMILY_A be the queue family that currently owns an image, and FAMILY_B be the queue family that the image is being transferred onto.

Both release and acquire barriers will have `src_queue_family_index` set to the FAMILY_A's index and `dst_queue_family_index` FAMILY_B's index. As for `subresource_range`, `old_layout` and `new_layout` must be equal between the two barriers, same thing for `offset` and `size` in buffer memory barriers. It is okay to perform an image layout transition during a queue ownership transfer, which would happen in between the release and acquire operations.

For release, set `dst_access_mask` to `NONE`, and no commands that use the image should occur after the release operation in the source queue (as the image contents will be undefined). The `dst_stage_mask` can be set to anything supported by the queue that aids setting the execution dependency between queues (using for example a semaphore).

```rust
let release = vk::ImageMemoryBarrier2 {
    ...
    dst_stage_mask: vk::PipelineStageFlags2::TRANSFER, // to semaphore
    dst_access_mask: vk::AccessFlags2::NONE,           // NONE for ownership release
    src_queue_family_index: family_a_index,            // FAMILY_A
    dst_queue_family_index: family_b_index,            // FAMILY_B
    ...
};
```

For acquire, set `src_access_mask` to `NONE`, and no commands that use the image should occur before the acquire operation in the source queue (as the image contents will be undefined). The `dst_stage_mask` should be set to the corresponding `dst_stage_mask` from the release operation or any other necessary to complete the memory dependency.

```rust
let src_acquire = vk::ImageMemoryBarrier2 {
    ...
    src_stage_mask: vk::PipelineStageFlags2::TRANSFER, // from semaphore
    src_access_mask: vk::AccessFlags2::NONE,           // NONE for ownership acquire
    src_queue_family_index: family_a_index,            // FAMILY_A
    dst_queue_family_index: family_b_index,            // FAMILY_B
    ...
};
```

# Example rundown

This example uses two queues from two different queue families, where one family only supports transfer operations and the other supports compute. The purpose is that the image clear operation is executed in the compute queue, while the copy from image to buffer operation is executed in the transfer queue.

A command pool must belong to only one queue family, so there has to be one pool for compute and one for transfer, each containing one command buffer.

### Recording the command buffers

In the compute buffer (`src/command_pools/compute.rs`):

- Prepare the image, switching its layout from `UNDEFINED` to `TRANSFER_DST_OPTIMAL` (the best layout for the next command). Make sure it completes before `CLEAR`, which is the pipeline stage of the next "clear image to a specific color" command, and memory is properly available.

- Execute the clear command. It takes a color value, which here is a constant defined in `main.rs`.

- Transition the image layout again, this time from `TRANSFER_DST_OPTIMAL` to `TRANSFER_SRC_OPTIMAL`. If the compute queue and the transfer queue are different (they can be the same if the device doesn't support separate compute and transfer queue families) perform a queue ownership release between the compute queue family and the transfer queue family at the same time as the layout transition.

In the transfer buffer (`src/command_pools/transfer.rs`):

- If compute and transfer queues are different, perform a queue ownership acquire and complete the queue ownership operation.

- Make a full image to buffer copy.

- Flush buffer contents to host (they will be then fully available in the device domain).

### Submitting the batches

In order to synchronize the commands between the command buffers and make sure the release and acquire operations run in order, a semaphore is used. When submitting the batches:

- The compute batch signals this semaphore when all commands in a specific stage finish. Here it is set to `TRANSFER`, which is also the stage indicated in `dst_stage_mask` on the release operation in the compute buffer.

- The transfer batch makes sure that all commands that are set to execute after `TRANSFER` only start after the semaphore is signaled. This includes the acquire operation and the copy (in case that the queue families are equal and the acquire is not executed).

A fence is created in order to wait on the transfer batch. There is no need to have another fence to wait on the compute batch, as the two batches are already synchronized between each other.

### Mapping and reading buffer contents

In order to actually read buffer contents, they have to be mapped first. This makes the contents accessible in virtual memory through a pointer. Buffers can remain mapped across batch submissions and the only reason to unmap a buffer is to get back the virtual space, which can help in case that other map operations start failing. It is an error to map an memory object that is already mapped and freeing a memory object implicitly unmaps its memory.

Before the map operation, the application also invalidates the range of memory about to mapped if the memory type doesn't have the `HOST_COHERENT` flag, in order to make memory from the device domain visible to the host. There is no difference if you do this when the buffer is already mapped, and mapping doesn't automatically perform the visibility operation.

After getting the pointer, the only thing that's needed is to transform it into a slice and send it to the `image::save_buffer` function.

```rust
let data = std::slice::from_raw_parts(ptr, buffer_size);
image::save_buffer(
    IMAGE_SAVE_PATH,
    data,
    IMAGE_WIDTH,
    IMAGE_HEIGHT,
    IMAGE_SAVE_TYPE,
);
```

## Cargo features

This example implements the following cargo features:

- `vl`: Enable validation layers.
- `load`: Load the system Vulkan Library at runtime.
- `link`: Link the system Vulkan Library at compile time.
- `graphics_family`: Create a graphics queue.
- `compute_family`: Create a compute queue.
- `transfer_family`: Create a transfer queue.

These are used in the following profiles:

 - default `dev`: enables `vl` and `load`.
 - `release-validation`: runs in --release with validation enabled.
 - `release-fast`: runs in --release with validation disabled.

For example:

`cargo run --profile release-fast`

You may pass a custom feature list by using `--no-default-features` and a list of features to 
enable, for example by passing `--features link,graphics_family`.

For more information about linking and loading check
[https://docs.rs/ash/latest/ash/struct.Entry.html](https://docs.rs/ash/latest/ash/struct.Entry.html).
