# Device creation

This example is a direct continuation of
[Instance creation](https://github.com/ZakStar17/ash-by-example/tree/main/src/bin/instance).
It covers physical device selection, logical device creation and queue retrieval.

The added files are `logical_device.rs` and `physical_device.rs` as well as their code in `main.rs`.

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
