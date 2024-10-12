# Instance creation

This example covers creating an instance and passing a list of instance extensions and layers. This also includes creating a Debug Messenger which can pass down messages from validation layers when they are enabled.

This example uses ash version `0.38`.

You can run this example with:

`RUST_LOG=debug cargo run`

## Beforehand

If you are new to Vulkan, read the initial part of the [Vulkan Guide](https://docs.vulkan.org/guide/latest/what_is_vulkan.html) and everything in the [Vulkan Tutorial](https://docs.vulkan.org/tutorial/latest/03_Drawing_a_triangle/00_Setup/01_Instance.html) up until the instance chapter. If you don't mind reading the specification, then also read [Fundamentals](https://docs.vulkan.org/spec/latest/chapters/fundamentals.html) and the [Initialization](https://docs.vulkan.org/spec/latest/chapters/initialization.html) chapter.

## Covered contents in a nutshell

### API Versions

Vulkan major versions may introduce new functionality that may not be backwards compatible or supported in some environments. An implementation (most commonly known as the graphics driver) may only support Vulkan up to a specific version. Creating an instance requires passing a specific version that your application wants to target and it has to be equal or lower to what is supported by the implementation.

For example, imagine your application wants to use functionality from the 1.2 version. In order to do that, you have to:

 - Query the version supported by the implementation, and check if it is higher or equal to 1.2.
 - Create an instance and pass the desired target version.
 - Query the version supported by one's physical device (for example one specific GPU), and check if it is also higher or equal to 1.2. Some users may have multiple devices with different versions and in some cases they may still be lower than what is supported by the implementation because the device just doesn't have the necessary capabilities.

Application may fallback to using older functionality if the newer one is not supported.

### Instance creation

An instance stores Vulkan application state and is used in most API calls that are not related to any specific device. 

Creating an instance takes the following parameters:

 - An API version: The desired target version that is supported by the implementation, as was explained in the previous subchapter. 
 - A `vk::ApplicationInfo`: Some implementations may use this information to label your application as belonging to a specific class.
 - A list of global extensions: Some functionality can only be called if an particular extension is enabled (which can only be done if the execution environment supports it). The list of extensions passed in instance creation are global, meaning they enable functionality that is not specific to any device.
 - A list of layers: These are external libraries that can be inserted in between the Vulkan loader. They usually don't directly affect the core functionality, and instead add separated logging, tracing and validation that can be easily toggled (for example to only be present in debug builds). Some layers also allow the application to be able to use some extensions that are not natively supported, but in turn are simulated by the layer.

### Enabling validation layers

Vulkan by design doesn't check for undefined behavior like missuses of the API or out of bounds memory accesses, or other performance and memory issues. In order to mitigate that, validation layers can be enabled in some builds to detect and communicate those issues to you. These layers are not part of the Vulkan API and may have to be installed separately, but in turn allows you to download and enable any more specific validation layers in a more customizable way.

Validation can be performance heavy, so generally only a subset of its functionality is enabled at a time. For example, some synchronization bugs can only occur in environments that are as fast as release builds because of timing issues, where in that case only the part of validation that is required to catch those specific bugs can be enabled. In release builds the validation layers can be completely disabled. 

The `VK_LAYER_KHRONOS_validation` layer is Vulkan's main validation layer and it can validate input and detect malpractices and other missuses of the API. It also contains some features related to debugging shader code and detecting synchronization issues. See https://docs.vulkan.org/guide/latest/development_tools.html#_vulkan_layers for more information and other info about other related validation layers.

This application configures some of `VK_LAYER_KHRONOS_validation` features programmatically, by passing an `vk::ValidationFeaturesEXT` struct during instance creation. However, it is easier to configure it using using the [Vulkan Configurator](https://vulkan.lunarg.com/doc/view/1.3.275.0/windows/vkconfig.html) (see [https://vulkan.lunarg.com/doc/view/1.3.275.0/windows/layer_configuration.html](https://vulkan.lunarg.com/doc/view/1.3.275.0/windows/layer_configuration.html)), where you can override the normal behavior by toggling checkboxes in a GUI instead.

This example uses a `vk::DebugUtilsMessengerEXT` object that can receive and parse messages from validation. It takes a callback function which in this example formats the messages and logs them using Rust's `log` crate. In order for messages to be received during instance creation, this object's creation info is also passed in the instance creation info `p_next` chain.

## Some code explanations

As the application is small, you may want to first take a look at `./src/main.rs` and try to follow the code as it is from here.

The file `./src/instance.rs` has all the code responsible for checking and creating the instance. Its main function is:

```rust
// (safety: extensions and layers should be valid cstrings)
fn create_instance_checked(
  entry: &ash::Entry,
  app_info: vk::ApplicationInfo,
  extensions: &[*const c_char],
  layers: &[*const c_char],
  p_next: *const c_void,
) -> Result<ash::Instance, InstanceCreationError>
```

Which checks if all layers and extensions are valid and the desired API version is supported, or returns an error otherwise. It also takes an optional `p_next` pointer that may add some extended functionality not tested by this function.

The `create_instance_checked` function is called by another function that is public and is used in main. In case that no validation layers are to be enabled, this other function simply passes default or empty parameters:

```rust
#[cfg(not(feature = "vl"))]
pub fn create_instance(entry: &ash::Entry) -> Result<ash::Instance, InstanceCreationError> {
  check_api_version(entry)?;

  let app_info = get_app_info();
  let extensions = [];
  let layers = [];
  create_instance_checked(entry, app_info, &extensions, &layers, ptr::null())
}
```

However, when validation layers are enabled, it takes some constants defined in main:

```rust
// validation layers names should be valid cstrings (not contain null bytes nor invalid characters)
#[cfg(feature = "vl")]
const VALIDATION_LAYERS: [&CStr; 1] = [c"VK_LAYER_KHRONOS_validation"];
#[cfg(feature = "vl")]
const ADDITIONAL_VALIDATION_FEATURES: [vk::ValidationFeatureEnableEXT; 2] = [
  vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
  vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
];
```

Checks if the layers exists, and enables the extra features by passing a `vk::ValidationFeaturesEXT` struct in the `p_next` field.

```rust
#[cfg(feature = "vl")]
pub fn create_instance(
  entry: &ash::Entry,
) -> Result<(ash::Instance, crate::validation_layers::DebugUtils), InstanceCreationError> {
  use crate::{
    validation_layers::{self, DebugUtils},
    ADDITIONAL_VALIDATION_FEATURES,
  };

  let app_info = get_app_info();

  let extensions = vec![ash::ext::debug_utils::NAME.as_ptr()];

  let layers_str = validation_layers::get_supported_validation_layers(entry)
    .map_err(|err| InstanceCreationError::OutOfMemory(err.into()))?;
  let layers: Vec<*const c_char> = layers_str.iter().map(|name| name.as_ptr()).collect();

  let debug_create_info = DebugUtils::get_debug_messenger_create_info();

  // enable/disable some validation features by passing a ValidationFeaturesEXT struct
  let additional_features = vk::ValidationFeaturesEXT {
    s_type: vk::StructureType::VALIDATION_FEATURES_EXT,
    p_next: &debug_create_info as *const vk::DebugUtilsMessengerCreateInfoEXT as *const c_void,
    enabled_validation_feature_count: ADDITIONAL_VALIDATION_FEATURES.len() as u32,
    p_enabled_validation_features: ADDITIONAL_VALIDATION_FEATURES.as_ptr(),
    disabled_validation_feature_count: 0,
    p_disabled_validation_features: ptr::null(),
    _marker: PhantomData,
  };

  let instance = create_instance_checked(
    entry,
    app_info,
    &extensions,
    &layers,
    &additional_features as *const vk::ValidationFeaturesEXT as *const c_void,
  )?;

  log::debug!("Creating Debug Utils");
  let debug_utils = DebugUtils::create(entry, &instance, debug_create_info)?;

  Ok((instance, debug_utils))
}
```

The previous function also creates a object called `DebugUtils`, which has the job of retrieving the messages written by the validation layers and forward them to the application normal logging implementation. Creating this debug messenger and making it work during instance creation is a bit convoluted as it requires passing the `vk::DebugUtilsMessengerCreateInfoEXT` struct as well as a special external function that does the actual message translation and forwarding.

```rust
// can be extensively customized
unsafe extern "system" fn vulkan_debug_utils_callback(
  message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
  message_type: vk::DebugUtilsMessageTypeFlagsEXT,
  p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
  _p_user_data: *mut c_void,
) -> vk::Bool32 {
  let types = match message_type {
    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "[General] ",
    vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "[Performance]\n",
    vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "[Validation]\n",
    _ => "[Unknown]\n",
  };
  let message = CStr::from_ptr((*p_callback_data).p_message);
  let message = format!("{}{}", types, message.to_str().unwrap());
  match message_severity {
    vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => log::debug!("{message}"),
    vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => log::warn!("{message}"),
    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => log::error!("{message}"),
    vk::DebugUtilsMessageSeverityFlagsEXT::INFO => log::info!("{message}"),
    _ => log::warn!("<Unknown>: {message}"),
  }

  vk::FALSE
}
```

This type of debugging can be very extensive depending on your needs. Check for example https://docs.vulkan.org/spec/latest/chapters/debugging.html#debugging-debug-messengers.

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
