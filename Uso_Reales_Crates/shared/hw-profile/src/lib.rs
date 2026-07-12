//! BMO Hardware Profile.
//!
//! Compile-time constants describing the target hardware. This crate is the
//! single source of truth for hardware capabilities — no other code should
//! use CPUID or device detection for build-time features.
//!
//! ## Usage
//!
//! ```ignore
//! use hw_profile::*;
//!
//! if ENABLE_AVX2 { /* use AVX2 code path */ }
//! if ENABLE_RDNA4_DRIVER { /* enable RDNA4 init */ }
//! let cores = CORE_COUNT;
//! ```
//!
//! ## Adding a new profile
//!
//! 1. Add `src/intel_core_ultra.rs` (or similar).
//! 2. Change the `mod` + `pub use` below to point at the new profile.
//! 3. Done — every user of `hw_profile::*` picks up the new constants.

#![no_std]

mod amd_ryzen_5_5600x;
pub use amd_ryzen_5_5600x::*;
