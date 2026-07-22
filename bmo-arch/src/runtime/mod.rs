//! # Ring 3 runtime helpers
//!
//! This is the first code that runs when a userland process is
//! spawned by the kernel. The kernel:
//!
//! 1. Allocates the process's address space (PML4, user half).
//! 2. Maps the shared estuary pages from
//!    `BootContext.channel_pages[]` into the process's address space.
//! 3. Places the process's `rsp` at the top of a user stack.
//! 4. Jumps to the process's `_start` with `rdi = &BootContextV1`.
//!
//! `_start` is provided by `bmo-rt` (the userspace runtime in
//! `platform/abi/bmo-rt/`). It calls
//! [`bmo_platform::runtime::boot`] to initialize the platform layer,
//! then jumps to the userland's `main`.
//!
//! After `boot()` returns, the rest of the process can use the
//! `bmo_platform` API as if it were running on any CPU — the
//! architecture is selected once, here, and never visible again.

// Sub-module declarations.
pub mod boot;

// Re-export the boot types so callers can `use
// bmo_platform::runtime::{PlatformInfo, BootContextV1, Estuaries}`.
pub use self::boot::{boot, BootContextV1, Estuaries, PlatformInfo};
