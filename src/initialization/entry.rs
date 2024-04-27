#[cfg(all(feature = "link", feature = "load"))]
compile_error!(
  "\
    Features \"link\" and \"load\" \
    were included at the same time. \
    Choose between \"load\" to load the Vulkan library \
    at runtime or \"link\" to link it while building the binary."
);

#[allow(unreachable_code)]
pub unsafe fn get_entry() -> ash::Entry {
  #[cfg(feature = "link")]
  return ash::Entry::linked();

  #[cfg(feature = "load")]
  return match ash::Entry::load() {
    Ok(entry) => entry,
    Err(err) => match err {
      ash::LoadingError::MissingEntryPoint(missing_entry_error) => {
        panic!(
          "Missing entry point when loading Vulkan library: {}",
          missing_entry_error
        )
      }
      ash::LoadingError::LibraryLoadFailure(load_error) => {
        panic!("Failed to load Vulkan Library: {:?}", load_error)
      }
    },
  };

  // panic will only happen if neither feature is enabled
  panic!(
    "No compile feature was included for accessing the Vulkan library.\n\
    Choose between \"load\" to load the Vulkan library \
    at runtime or \"link\" to link it while building the binary."
  );
}
