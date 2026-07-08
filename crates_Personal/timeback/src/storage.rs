//! `timeback::storage` — Backend de almacenamiento para snapshots.
//!
//! v1.8.8: RAM only.
//! v1.9: NVRAM persistence — each checkpoint's snapshot is written to a
//! UEFI NVRAM variable (256 bytes per var, max 8 vars = 2 KiB). The
//! callback is provided by the kernel via `register_nvram_sink`.

#![allow(dead_code)]

/// Tamaño máximo del storage en bytes (16 MB por ahora).
pub const STORAGE_CAP: usize = 16 * 1024 * 1024;

/// NVRAM variable prefix for TimeBack checkpoints.
pub const NVRAM_PREFIX: &str = "BMOTBKP";

/// Max bytes per NVRAM variable (UEFI safety).
pub const NVRAM_CHUNK: usize = 192;

/// Max number of NVRAM variables (BMOTBKP0..BMOTBKP7).
pub const NVRAM_VARS_MAX: u32 = 8;

static mut USED: usize = 0;

/// Callback type for writing a NVRAM variable. Set by kernel.
type SetVarFn = fn(name: &str, data: &[u8]);
static mut SET_VAR: Option<SetVarFn> = None;

/// Register the kernel's NVRAM write callback. Called once at boot.
pub fn register_nvram_sink(f: SetVarFn) {
    unsafe { SET_VAR = Some(f); }
}

/// Build NVRAM variable name for a chunk index.
pub fn var_name(idx: u32) -> alloc::string::String {
    let mut s = alloc::string::String::from(NVRAM_PREFIX);
    s.push_str(&alloc::format!("{}", idx));
    s
}

/// Write a snapshot to NVRAM (chunked across multiple variables).
/// Returns true on success.
pub fn persist_to_nvram(id: u32, snapshot_bytes: &[u8]) -> bool {
    let cb = unsafe { match SET_VAR { Some(f) => f, None => return false } };
    let total = snapshot_bytes.len();
    let mut offset = 0;
    let mut var_idx = 0u32;
    while offset < total && var_idx < NVRAM_VARS_MAX {
        let end = core::cmp::min(offset + NVRAM_CHUNK, total);
        let chunk = &snapshot_bytes[offset..end];
        let name = var_name(var_idx + (id % 4) * NVRAM_VARS_MAX);
        cb(&name, chunk);
        offset = end;
        var_idx += 1;
    }
    true
}

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
