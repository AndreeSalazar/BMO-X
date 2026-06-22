//! `timeback::storage` — Backend de almacenamiento para snapshots.
//!
//! v1.8.8: RAM only. En v1.9, USB/FS con append seguro.

#![allow(dead_code)]

/// Tamaño máximo del storage en bytes (16 MB por ahora).
pub const STORAGE_CAP: usize = 16 * 1024 * 1024;

static mut USED: usize = 0;

pub fn init() {
    unsafe { USED = 0; }
}

/// Bytes usados.
pub fn used_bytes() -> usize { unsafe { USED } }

/// Capacidad total.
pub fn capacity() -> usize { STORAGE_CAP }

/// ¿Hay espacio para `n` bytes más?
pub fn can_fit(n: usize) -> bool { n + unsafe { USED } <= STORAGE_CAP }

/// Reserva `n` bytes (debe llamarse después de `can_fit`).
pub unsafe fn reserve(n: usize) { USED += n; }
