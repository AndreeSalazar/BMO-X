// Shared boot ABI for the BMO kernel — re-exported from the
// workspace so userland crates can depend on it without knowing
// the relative path to `../Ultra_kernel/boot_context/`.

pub use boot_context::*;
