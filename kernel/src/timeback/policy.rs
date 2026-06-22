//! `timeback::policy` — Reglas de retención de snapshots.

#![allow(dead_code)]

/// # máximo de checkpoints a retener.
pub const MAX_CHECKPOINTS: usize = 32;

/// # máximo de entradas en el journal.
pub const MAX_JOURNAL: usize = 256;

/// ¿Cuántos días se retienen los snapshots por defecto?
pub const DEFAULT_RETENTION_DAYS: u32 = 7;

pub fn init() {}
