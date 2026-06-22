//! `gpu/` — GPU kernel interface (Ring 0 side).
//!
//! v1.8.8: skeleton. The actual GPU driver lives in
//! `vendor/amd/gpu/rdna4/` (Phase 4 of the Triple A roadmap). This
//! module provides the minimal interface that BMO GPU uses to talk
//! to the underlying vendor driver.

pub mod handles;
pub mod kernel_device;
pub mod syscalls;

pub use handles::*;
pub use kernel_device::*;
pub use syscalls::*;
