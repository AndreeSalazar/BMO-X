//! `values` — tipos valor con semántica propia del BMO ABI.
//!
//! - [`string`]   — `BmoStr`, `BmoString`, ASCII helpers (UTF-8 con length).
//! - [`time`]     — `BmoInstant`, `BmoDuration` (sustituye `time_t`/`timespec`).
//! - [`reflect`]  — reflection runtime sobre cualquier BEF cargado.

pub mod string;
pub mod time;
pub mod reflect;
pub mod net;
