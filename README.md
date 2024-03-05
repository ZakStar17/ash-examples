# Instance creation

This example covers creating an instance, with or without validation layers enabled.

You can run this example with:

`RUST_LOG=debug cargo run`

## Instance creation info

An instance stores Vulkan application state and can be used to make API calls that are not related to any specific device. 

Creating an instance takes the following parameters:

 - A `vk::ApplicationInfo`: Some implementations may use this to classify the application.
 - A list of global extensions: Some functionality is included in the API header, however it may not be always supported, and so it is only enabled by the respective extensions. The list of extensions passed in instance creation refer to global functionality (which is not device specific).
 - A list of layers: These enable functionality outside of the Vulkan specification (which can include some extensions), and execute in between the application API call and the actual command. They usually don't directly affect the core functionality, and instead add separated logging, tracing and validation functionality that can be easily toggled (for example to only be present in debug builds). Some layers also allow the application to be able to use some extensions that are not natively supported, but in turn are simulated by the layer.

## Enabling validation layers

The `VK_LAYER_KHRONOS_validation` layer can validate input and detect malpractices and other missuses of the API. Validation can be performance heavy, so generally only a subset of its functionality is enabled at a time and it all is completely disabled in release builds.

It can be configured extensively by the application or by using the [Vulkan Configurator](https://vulkan.lunarg.com/doc/view/1.3.275.0/windows/vkconfig.html) (see [https://vulkan.lunarg.com/doc/view/1.3.275.0/windows/layer_configuration.html](https://vulkan.lunarg.com/doc/view/1.3.275.0/windows/layer_configuration.html)). This example demonstrates how can you enable additional validation features programmatically by passing an `vk::ValidationFeaturesEXT` struct during instance creation.

This example uses a `vk::DebugUtilsMessengerEXT` object that can receive and parse messages from validation. It takes a callback function which in this example formats the messages and logs them using Rust's `log` crate. In order for messages to be received during instance creation, this object's creation info is passed in the instance creation info `p_next` chain.

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
