# Instance creation

This example covers creating an instance, with or without validation layers enabled.

An instance stores Vulkan application state and enables making API calls that are not related to any specific device. 

When creating an instance, some additional information can be passed:

 - A `vk::ApplicationInfo`: It can be used by some implementations to classify the behavior of the application.
 - A list of global extensions: These enable non device specific functionality that works at the same level as the rest of the API. This can include new objects or commands however, once enabled, these function in the same way as the core API.
 - A list of layers: These enable functionality outside of the Vulkan specification, and execute in between the application API call and the actual command. They usually don't directly affect the code API, and instead add logging, tracing and validation functionality that can be easily toggled (for example to only be present in debug builds). Some layers also allow the application to enable some extensions that are not natively supported, but can instead be emulated by the layer.

The module for the validation layers is only compiled when the `vl` feature is enabled.

You can run this example with:

`RUST_LOG=debug cargo run`

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
