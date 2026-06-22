//! `profile/active.rs` — Re-exports the active hardware profile.
//!
//! This is the **only** file the rest of the kernel should use to query
//! hardware capabilities. When the build target changes (e.g. to
//! Ryzen 9000 + RDNA4), this file is updated to point to the new
//! profile — no other file needs to change.
//!
//! ## Usage
//!
//! ```ignore
//! use crate::profile::active::*;
//!
//! if ENABLE_AVX2 { /* use AVX2 code path */ }
//! if ENABLE_RDNA4_DRIVER { /* enable RDNA4 init */ }
//! let cores = CORE_COUNT;
//! ```
//!
//! v1.8.8: the active profile is `amd_ryzen_5_5600x`.

pub use super::amd_ryzen_5_5600x::*;
