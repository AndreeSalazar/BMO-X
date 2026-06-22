//! v2.0 — Handle table global.
//!
//! v1.8.8: usa el `BmoHandle` canónico de `bmo_abi` (no redefine).
//! La tabla de handles es la **implementación interna** del kernel;
//! los handles que cruzan la frontera ring 0↔ring 3 son `BmoHandle` ABI.

#![allow(dead_code)]

use crate::bmo_abi::fundamentals::handle::BmoHandle;

// ─── Tipos de handle (códigos de kind) ───────────────────────────────

/// Códigos de kind (sincronizados con `HandleKind::code()`).
pub mod kind {
    pub const WINDOW: u8 = 1;
    pub const DC: u8 = 2;
    pub const SURFACE: u8 = 3;
    pub const TIMER: u8 = 4;
    pub const CLASS: u8 = 5;
}

/// Una entrada de la tabla de handles.
pub struct HandleEntry {
    pub kind: u8,
    pub generation: u16,
    pub used: bool,
    /// Puntero al objeto subyacente (BmoWindow, BmoSurface, ...).
    pub data_ptr: usize,
}

pub const MAX_HANDLES: usize = 1024;

pub struct HandleTable {
    pub entries: [HandleEntry; MAX_HANDLES],
    pub alloc_count: u32,
}

impl HandleTable {
    pub const fn new() -> Self {
        const EMPTY: HandleEntry = HandleEntry {
            kind: 0,
            generation: 0,
            used: false,
            data_ptr: 0,
        };
        Self {
            entries: [EMPTY; MAX_HANDLES],
            alloc_count: 0,
        }
    }

    pub fn init(&mut self) {
        for e in self.entries.iter_mut() {
            e.kind = 0;
            e.generation = 0;
            e.used = false;
            e.data_ptr = 0;
        }
        self.alloc_count = 0;
    }

    /// Reserva un slot para un objeto de tipo `kind`.
    /// Devuelve el `BmoHandle` canónico ABI.
    pub fn alloc(&mut self, kind: u8, data_ptr: usize) -> Option<BmoHandle> {
        for (i, e) in self.entries.iter_mut().enumerate() {
            if !e.used {
                e.used = true;
                e.kind = kind;
                e.generation = e.generation.wrapping_add(1);
                e.data_ptr = data_ptr;
                self.alloc_count += 1;
                // Layout BmoHandle: tag(1) | kind(7) | gen(16) | index(40)
                // Usamos kind=code del usuario + generation propia + index=i.
                let h = (kind as u64) << 56
                      | (e.generation as u64) << 40
                      | (i as u64);
                return Some(BmoHandle(h));
            }
        }
        None
    }

    /// Libera un handle. Devuelve `true` si era válido.
    pub fn free(&mut self, h: BmoHandle) -> bool {
        if h.is_null() { return false; }
        let slot = h.index() as usize;
        let gen = h.generation();
        if slot >= MAX_HANDLES { return false; }
        let e = &mut self.entries[slot];
        if !e.used || e.generation != gen { return false; }
        e.used = false;
        e.data_ptr = 0;
        self.alloc_count -= 1;
        true
    }

    /// Resuelve un handle. Devuelve `Some((kind, data_ptr))` si es válido.
    pub fn resolve(&self, h: BmoHandle) -> Option<(u8, usize)> {
        if h.is_null() { return None; }
        let slot = h.index() as usize;
        let gen = h.generation();
        if slot >= MAX_HANDLES { return None; }
        let e = &self.entries[slot];
        if !e.used || e.generation != gen { return None; }
        Some((e.kind, e.data_ptr))
    }
}

// ─── Type aliases (compatibilidad) ──────────────────────────────────
pub type BmoWindowHandle = BmoHandle;
pub type BmoSurfaceHandle = BmoHandle;
pub type BmoClassHandle = BmoHandle;
pub type BmoTimerHandle = BmoHandle;
pub type BmoDcHandle = BmoHandle;
