//! v2.0 — Handle table global.
//!
//! Cada entry es `(kind, generation, kind_specific_data)`. Un `bmo_handle_t`
//! es `(kind: u8, slot: u24)`. La `generation` se incrementa en cada
//! `destroy` para que los handles obsoletos se detecten.

#![allow(dead_code)]

#[allow(unused_imports)]
use super::window::BmoWindow;
#[allow(unused_imports)]
use super::surface::BmoSurface;

/// Tipos de handle (synchronizados con `BMO_HANDLE_KIND_*`).
pub mod kind {
    pub const WINDOW: u8 = 1;
    pub const DC: u8 = 2;
    pub const SURFACE: u8 = 3;
    pub const TIMER: u8 = 4;
    pub const CLASS: u8 = 5;
}

/// Una entrada de la tabla de handles. Usa una enum-like union manual:
/// el `data_ptr` apunta a un objeto del tipo que indica `kind`. El
/// GC de handles se hace por `destroy` (incrementa `generation`).
pub struct HandleEntry {
    pub kind: u8,
    pub generation: u16,
    pub used: bool,
    /// Puntero al objeto subyacente (BmoWindow, BmoSurface, ...).
    /// Codificado como usize para mantener la union a bajo nivel.
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

    /// Reserva un slot para un objeto de tipo `kind`. Devuelve (slot, generation).
    pub fn alloc(&mut self, kind: u8, data_ptr: usize) -> Option<(u32, u16)> {
        for (i, e) in self.entries.iter_mut().enumerate() {
            if !e.used {
                e.used = true;
                e.kind = kind;
                e.generation = e.generation.wrapping_add(1);
                e.data_ptr = data_ptr;
                self.alloc_count += 1;
                return Some((i as u32, e.generation));
            }
        }
        None
    }

    /// Valida y devuelve el puntero al objeto. Devuelve None si la
    /// generation no coincide o el slot está libre.
    pub fn lookup(&self, slot: u32, kind: u8, generation: u16) -> Option<usize> {
        let e = self.entries.get(slot as usize)?;
        if !e.used || e.kind != kind || e.generation != generation { return None; }
        Some(e.data_ptr)
    }

    /// Libera un slot e incrementa la generation (invalida handles viejos).
    /// El caller debe encargarse de liberar el objeto subyacente.
    pub fn free(&mut self, slot: u32) -> bool {
        if let Some(e) = self.entries.get_mut(slot as usize) {
            if !e.used { return false; }
            e.used = false;
            e.data_ptr = 0;
            e.generation = e.generation.wrapping_add(1);
            self.alloc_count = self.alloc_count.saturating_sub(1);
            true
        } else { false }
    }

    /// Helper: busca un objeto de tipo ventana por su `data_ptr`. Se usa
    /// cuando tenemos un puntero a `BmoWindow` y queremos encontrar su slot
    /// para operaciones de invalidación.
    pub fn slot_of(&self, data_ptr: usize) -> Option<u32> {
        for (i, e) in self.entries.iter().enumerate() {
            if e.used && e.data_ptr == data_ptr { return Some(i as u32); }
        }
        None
    }
}

/// Handle público que se entrega a Ring 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoHandle {
    pub kind: u8,
    pub slot: u32,
    pub generation: u16,
}

impl BmoHandle {
    pub const INVALID: BmoHandle = BmoHandle { kind: 0, slot: 0xFFFFFFFF, generation: 0 };

    pub fn is_valid(&self) -> bool { self.slot != 0xFFFFFFFF }

    pub fn encode(self) -> u64 {
        ((self.kind as u64) << 56) | ((self.slot as u64) << 16) | (self.generation as u64)
    }

    pub fn decode(v: u64) -> Self {
        BmoHandle {
            kind: ((v >> 56) & 0xFF) as u8,
            slot: ((v >> 16) & 0x00FFFFFF) as u32,
            generation: (v & 0xFFFF) as u16,
        }
    }
}

// Los siguientes son type aliases para reducir el ruido en el resto de archivos.
pub type BmoWindowHandle = BmoHandle;
pub type BmoSurfaceHandle = BmoHandle;
pub type BmoClassHandle = BmoHandle;
pub type BmoTimerHandle = BmoHandle;
pub type BmoDcHandle = BmoHandle;
