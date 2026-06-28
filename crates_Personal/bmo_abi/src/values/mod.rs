//! `values` — tipos valor con semántica propia del BMO ABI.
//!
//! - [`time`]     — `BmoInstant`, `BmoDuration` (sustituye `time_t`/`timespec`).
//! - [`math`]     — sqrt, sin, cos, pow (sustituye `libm`).
//! - [`hash`]     — FNV-1a, CRC32c, CRC32 (sustituye hashes ad-hoc).
//! - [`net`]      — `BmoIpv4Addr`, `BmoIpv6Addr`, `BmoSocketAddr`.
//! - [`reflect`]  — reflexión sobre tipos BEF cargados.

pub mod time;
pub mod math;
pub mod hash;
pub mod net;
pub mod reflect;
