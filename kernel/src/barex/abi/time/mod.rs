//! `time` — manejo de tiempo BMO ABI.
//!
//! Reemplaza `time_t`, `timespec`, `GetTickCount`, `QueryPerformanceCounter`
//! con un único `BmoInstant` monotónico de nanosegundos desde boot.

pub mod instant;
pub mod duration;

