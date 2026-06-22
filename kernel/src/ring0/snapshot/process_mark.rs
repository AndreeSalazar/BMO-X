//! `ring0::snapshot::process_mark` — Marcas de procesos vivos.

#![allow(dead_code)]

const MAX_PROCS: usize = 64;

static mut LIVE: [u64; MAX_PROCS] = [0; MAX_PROCS]; // 0 = empty, else = CR3
static mut COUNT: usize = 0;

pub fn init() {
    unsafe {
        for p in &mut LIVE { *p = 0; }
        COUNT = 0;
    }
}

/// Registra un proceso vivo (por CR3).
pub fn add(cr3: u64) {
    unsafe {
        for slot in &mut LIVE {
            if *slot == 0 { *slot = cr3; COUNT += 1; return; }
        }
    }
}

/// Quita un proceso (al exit).
pub fn remove(cr3: u64) {
    unsafe {
        for slot in &mut LIVE {
            if *slot == cr3 { *slot = 0; COUNT = COUNT.saturating_sub(1); return; }
        }
    }
}

/// # de procesos vivos.
pub fn count() -> usize { unsafe { COUNT } }
