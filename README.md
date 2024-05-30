# Device creation

This example is a direct continuation of
[Instance creation](https://github.com/ZakStar17/ash-by-example/tree/main/instance).
It covers physical device selection, logical device creation and queue retrieval.

You can run this example with:

`RUST_LOG=debug cargo run`

## Overview

### Physical device selection

Physical devices are anything physical that supports Vulkan, like a dedicated GPU or a CPU, and are often very different in terms of performance and memory availability.

Most of a physical device information and limitations can be queried by the API even before the device is initialized. This can be used to "plan ahead" the program execution by, for example, selecting which Vulkan objects should be used and how memory should be managed.

Physical device selection can be a convoluted process and the decision of which device to use should mostly come down to the user. However, it is still a good idea to test that the chosen physical device(s) supports all application requirements and detect any errors that may cause objects to fail during program execution.

### Logical device

A logical device (usually just called a "device") is a logical representation of a specific physical device and it is the object that is the most interacted with through the program execution. In some cases a logical device may also correspond instead to a device group, which is a collection of multiple physical devices with similar characteristics that share some specific workload or memory.

Vulkan enables the use of multiple devices simultaneously but it has some limitations as devices usually cannot share dedicated memory and so data may have to be stored in RAM which may be slow specially for dedicated cards.

Creating a device takes a list of device extensions as well as a list of features to be enabled.

- Device extensions provide device-level functionality that extends the core API similarly to instance extensions, but where commands take device objects instead of instance ones.

- Features describe functionality that may not be supported on all devices or implementations (but can still be emulated by layers). These are optional and may enable the use of some particular object or command, and should be chosen with care.

For example, these are two features that are commonly enabled:

- timeline_semaphore: Enables the use of semaphores (synchronization primitives that can provide barriers between queues) that, instead of being binary, provide a 64bit integer that may be specifically waited upon by the host or a queue submission.

- synchronization2: Provides an extension of pipeline stages and access flags.

### Queues

All Vulkan work must be submitted in batches to specific queues that run the command buffers independently from one another. As per usual, queues have different capabilities and so are grouped into sets called "queue families". These sets are important as it doesn't matter to which queue a batch is submitted as long as the queues belong to the same family for which the batch was originally created.

Physical devices will usually contain a few queue families with different subsets of capabilities, ranging from supporting graphics, compute, video decode, video encode, protected memory management, sparse memory management, and transfer operations, as well as some enabled by extensions, like presenting to a screen. These capabilities have some implicit rules:

- Queues that support graphics operations will implicitly support compute and transfer operations.

- Queues that support compute operations will implicitly support transfer operations.

Queues that belong to a queue family with a fewer subset of capabilities will usually be specialized to perform faster operations on commands that are supported. For example, a queue that only supports transfer operations will usually perform copy operations faster that a more general queue that can also perform graphics commands.

Queues can execute work simultaneously with other queues and that will often be the case unless explicit synchronization objects are used. Also, objects that reside in device memory like images and buffers will often be owned by a specific queue family, in which case accessing them by a queue belonging to another family will most of the time require specific ownership transfer and synchronization commands. Memory objects can be created to be allowed to be accessed simultaneously by different queue families, but the order in which that occurs will still have to be managed internally which may include some performance penalities.

Your program will need to retrieve all queues that will be used beforehand during device creation. You can also use a array of normalized priority values that may signal a driver to prioritize work submitted in specific queues, but can also lead to queue starvation, where work in a specific queue is never executed as it is only scheduled to start after other queues with higher priority that always have work.

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
