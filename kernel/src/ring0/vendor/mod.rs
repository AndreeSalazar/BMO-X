//! `vendor/` — Hardware vendor profiles.
//!
//! v1.8.8: only `amd/` is implemented. Future: `intel/`, `arm/`, etc.
//!
//! The active vendor is implicitly `amd` for this build (no abstraction
//! layer — see `profile::active`).

pub mod amd;
