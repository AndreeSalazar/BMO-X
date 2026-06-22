//! `cabina::event` — Eventos + caja negra circular.
//!
//! Caja negra en RAM: los últimos 256 eventos. Si el sistema cae
//! antes de que el spool persistente funcione, todavía tenemos
//! los últimos eventos en RAM (Ring 0 los puede leer via snapshot).

#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, AtomicU32, AtomicBool, Ordering};
use core::fmt;
use alloc::string::String;

use crate::cabina::BLACKBOX_CAP;

/// Severidad de un evento.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Severity {
    /// Información de boot/operación normal.
    Info    = 0,
    /// Mensaje de trace (debugging fino).
    Trace   = 1,
    /// Advertencia (no fatal).
    Warning = 2,
    /// Fault (error recuperable, e.g. #GP en syscall).
    Fault   = 3,
    /// Panic (no recuperable, e.g. triple fault incipiente).
    Panic   = 4,
}

impl Severity {
    pub fn name(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Trace => "TRACE",
            Self::Warning => "WARN",
            Self::Fault => "FAULT",
            Self::Panic => "PANIC",
        }
    }

    pub fn color(self) -> u32 {
        match self {
            Self::Info => 0xFFCCCCCC,    // gris claro
            Self::Trace => 0xFF888888,   // gris oscuro
            Self::Warning => 0xFFFFFF00, // amarillo
            Self::Fault => 0xFFFF8800,    // naranja
            Self::Panic => 0xFFFF0000,    // rojo
        }
    }
}

/// Un evento de la cabina.
#[derive(Clone, Debug)]
pub struct Event {
    pub seq: u64,
    pub tick_ns: u64,
    pub severity: Severity,
    pub module: String,    // "boot", "fs", "lang", "kbc", "BMO", ...
    pub msg: String,
    pub value: u64,         // valor numérico opcional (addr, code, etc.)
}

impl Event {
    pub const fn empty() -> Self {
        Self {
            seq: 0,
            tick_ns: 0,
            severity: Severity::Info,
            module: String::new(),
            msg: String::new(),
            value: 0,
        }
    }
    pub fn new(severity: Severity, module: &str, msg: &str) -> Self {
        let mut e = Self::empty();
        e.severity = severity;
        e.module = String::from(module);
        e.msg = String::from(msg);
        e
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {} {}",
               self.severity.name(),
               self.module,
               self.msg,
               if self.value != 0 { alloc::format!("(0x{:x})", self.value) } else { String::new() })
    }
}

// ─── Buffer circular ──────────────────────────────────────────────

/// Caja negra circular: 256 eventos en RAM.
pub struct Blackbox {
    events: [Event; BLACKBOX_CAP],
    head: AtomicU32,    // posición de escritura
    count: AtomicU32,   // número de eventos (cap a BLACKBOX_CAP)
    seq: AtomicU64,     // sequence number monotónico
    initialized: AtomicBool,
}

static mut BLACKBOX: Blackbox = Blackbox {
    events: [const { Event::empty() }; BLACKBOX_CAP],
    head: AtomicU32::new(0),
    count: AtomicU32::new(0),
    seq: AtomicU64::new(0),
    initialized: AtomicBool::new(false),
};

pub mod buffer {
    use super::*;

    /// Inicializa la caja negra.
    pub fn init() {
        unsafe {
            BLACKBOX.initialized.store(true, Ordering::SeqCst);
            BLACKBOX.head.store(0, Ordering::SeqCst);
            BLACKBOX.count.store(0, Ordering::SeqCst);
            BLACKBOX.seq.store(0, Ordering::SeqCst);
        }
    }

    /// Empuja un evento a la caja negra.
    pub fn push(ev: &Event) {
        unsafe {
            if !BLACKBOX.initialized.load(Ordering::Relaxed) { return; }
            let pos = BLACKBOX.head.fetch_add(1, Ordering::Relaxed) as usize;
            let pos = pos % BLACKBOX_CAP;
            let mut stored = ev.clone();
            stored.seq = BLACKBOX.seq.fetch_add(1, Ordering::Relaxed);
            stored.tick_ns = {
                let tsc = crate::cpu::rdtsc();
                let freq = crate::cpu::tsc_per_sec();
                if freq == 0 { 0 } else { tsc.wrapping_mul(1_000_000_000) / freq }
            };
            let _ = stored;
            BLACKBOX.events[pos] = stored;
            // Incrementar count solo si no hemos llegado al máximo.
            let c = BLACKBOX.count.load(Ordering::Relaxed);
            if c < BLACKBOX_CAP as u32 {
                BLACKBOX.count.store(c + 1, Ordering::Relaxed);
            }
        }
    }

    /// Obtiene el evento más reciente (índice 0).
    pub fn latest() -> Option<Event> {
        unsafe {
            if !BLACKBOX.initialized.load(Ordering::Relaxed) { return None; }
            let count = BLACKBOX.count.load(Ordering::Relaxed);
            if count == 0 { return None; }
            // El último insertado está en head-1.
            let pos = (BLACKBOX.head.load(Ordering::Relaxed) as usize).wrapping_sub(1) % BLACKBOX_CAP;
            Some(BLACKBOX.events[pos].clone())
        }
    }

    /// Itera sobre los últimos N eventos (del más reciente al más viejo).
    pub fn last(n: usize) -> alloc::vec::Vec<Event> {
        unsafe {
            if !BLACKBOX.initialized.load(Ordering::Relaxed) { return alloc::vec::Vec::new(); }
            let count = BLACKBOX.count.load(Ordering::Relaxed) as usize;
            let n = n.min(count);
            let mut out = alloc::vec::Vec::with_capacity(n);
            let mut head = BLACKBOX.head.load(Ordering::Relaxed) as usize;
            for _ in 0..n {
                if head == 0 { head = BLACKBOX_CAP; }
                head -= 1;
                out.push(BLACKBOX.events[head].clone());
            }
            out
        }
    }

    /// Itera sobre todos los eventos (del más reciente al más viejo).
    pub fn iter() -> alloc::vec::Vec<Event> {
        last(BLACKBOX_CAP)
    }

    /// Número de eventos almacenados.
    pub fn count() -> usize {
        unsafe { BLACKBOX.count.load(Ordering::Relaxed) as usize }
    }
}

