//! `profile/` — Hardware profile selection.
//!
//! A "profile" describes what hardware is active for THIS build of
//! FastOS. It is the single source of truth for hardware capabilities.
//!
//! v1.8.8: only `amd_ryzen_5_5600x` is active. Future: zen4, zen5,
//! intel_core_ultra, etc.

pub mod amd_ryzen_5_5600x;
pub mod active;
