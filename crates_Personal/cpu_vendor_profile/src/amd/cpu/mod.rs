//! `vendor/amd/cpu/` — CPU profiles from AMD.
//!
//! v1.8.8: only `zen3/` (Ryzen 5 5600X) is implemented.
//! Future profiles: zen4 (Ryzen 7000 series), zen5 (Ryzen 9000).
//!
//! Each profile lives in its own submodule. The active profile is
//! selected by `crate::profile::active`.

pub mod zen3;
