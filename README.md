# Instance creation

This example covers creating an instance, with or without validation layers enabled.

An instance stores Vulkan application state and can be used to make API calls that are not related to any specific device. 

When creating an instance, some additional information can be passed:

 - A `vk::ApplicationInfo`: Some implementations may use this to classify the application.
 - A list of global extensions: Some functionality is included in the API header, however it may not be supported. A list of extensions is used to enable part of this functionality, and the ones passed here enable the global extensions, that are not device specific.
 - A list of layers: These enable functionality outside of the Vulkan specification (which can include some extensions), and execute in between the application API call and the actual command. They usually don't directly affect the core functionality, and instead add separated logging, tracing and validation functionality that can be easily toggled (for example to only be present in debug builds). Some layers also allow the application to be able to use some extensions that are not natively supported, but in turn are emulated by the layer.

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
