//! `bmo_abi::clock` — Contrato de tiempo (high-level, syscalls + tipos).
//!
//! Este módulo es un **índice** de los syscalls relacionados
//! (en `crate::bmo_abi::syscalls`) y un **re-export** de los tipos
//! definidos en `crate::bmo_abi::values::time`.
//!
//! ## Garantías
//!
//! - `BmoInstant::now()` es **monotónico**: nunca retrocede.
//! - No es afectado por NTP ni por el usuario.
//! - Resolución: TSC del CPU (~ns en hardware moderno).
//! - `BmoDuration` es siempre **no-negativo**.
//!
//! ## Syscalls (ver `syscalls/mod.rs`)
//!
//! - `NR_TIME_NOW_NS` (0x150) → `bmo_time_now_ns() -> u64`
//! - `NR_TIME_NOW_US` (0x151) → `bmo_time_now_us() -> u64`
//! - `NR_TIME_SLEEP_NS` (0x152) → `bmo_time_sleep_ns(ns: u64)`
//! - `NR_TIME_SLEEP_MS` (0x153) → `bmo_time_sleep_ms(ms: u64)`

#![allow(dead_code)]

pub use crate::bmo_abi::values::time::{BmoInstant, BmoDuration};

// ─── Helpers de conversión ─────────────────────────────────────────

/// Constantes de conversión.
pub const NS_PER_US: u64 = 1_000;
pub const US_PER_MS: u64 = 1_000;
pub const MS_PER_S:  u64 = 1_000;
pub const NS_PER_MS: u64 = NS_PER_US * US_PER_MS;
pub const NS_PER_S:  u64 = NS_PER_MS * MS_PER_S;

/// Convierte nanosegundos a un `BmoDuration`.
#[inline]
pub const fn ns_to_duration(ns: u64) -> BmoDuration { BmoDuration::from_ns(ns) }

/// Convierte microsegundos a un `BmoDuration`.
#[inline]
pub const fn us_to_duration(us: u64) -> BmoDuration { BmoDuration::from_us(us) }

/// Convierte milisegundos a un `BmoDuration`.
#[inline]
pub const fn ms_to_duration(ms: u64) -> BmoDuration { BmoDuration::from_ms(ms) }

/// Convierte segundos a un `BmoDuration`.
#[inline]
pub const fn s_to_duration(s: u64) -> BmoDuration { BmoDuration::from_secs(s) }
