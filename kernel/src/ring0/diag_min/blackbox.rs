//! `ring0::diag_min::blackbox` — Blackbox mínima fija (sin allocs).
//!
//! v1.8.8: usa un buffer estático de 32 entradas para los eventos más
//! críticos. Se sobrescribe en ring buffer. Sin allocs.

#![allow(dead_code)]

use super::BLACKBOX_MIN;

#[derive(Clone, Copy)]
pub struct MiniEvent {
    pub severity: u8,
    pub seq: u64,
    pub module: u32,  // puntero a string estático
    pub msg: u32,     // puntero a string estático
    pub value: u64,
}

const EMPTY: MiniEvent = MiniEvent {
    severity: 0, seq: 0, module: 0, msg: 0, value: 0,
};

static mut BUF: [MiniEvent; BLACKBOX_MIN] = [EMPTY; BLACKBOX_MIN];
static mut HEAD: usize = 0;

pub fn init() { unsafe { HEAD = 0; } }

/// Empuja un evento a la blackbox mínima.
pub fn push(severity: u8, seq: u64, module: u32, msg: u32, value: u64) {
    unsafe {
        BUF[HEAD] = MiniEvent { severity, seq, module, msg, value };
        HEAD = (HEAD + 1) % BLACKBOX_MIN;
    }
}

/// # de eventos en la blackbox.
pub fn count() -> usize { BLACKBOX_MIN }
