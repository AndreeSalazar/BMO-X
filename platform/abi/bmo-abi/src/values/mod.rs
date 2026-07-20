//! `values` — tipos valor con semántica propia del BMO ABI.
//!
//! - [`time`]     — `BmoInstant`, `BmoDuration` (sustituye `time_t`/`timespec`).
//! - [`clock`]    — `BmoClockId`, `sleep`, `sleep_until` (sustituye `clock_gettime`).
//! - [`uuid`]     — `BmoUuid` 128-bit (RFC 4122, sustituye `GUID`/`uuid_t`).
//! - [`version`]  — `BmoVersion` semver (major.minor.patch).
//! - [`math`]     — sqrt, sin, cos, pow (sustituye `libm`).
//! - [`hash`]     — FNV-1a, CRC32c, CRC32 (sustituye hashes ad-hoc).
//! - [`net`]      — `BmoIpv4Addr`, `BmoIpv6Addr`, `BmoSocketAddr`.
//! - [`reflect`]  — reflexión sobre tipos BEF cargados.

pub mod clock;
pub mod hash;
pub mod math;
pub mod net;
pub mod reflect;
pub mod time;
pub mod uuid;
pub mod version;
