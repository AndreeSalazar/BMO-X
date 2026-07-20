//! `capability` — BmoCapability, sistema de capacidades del BMO ABI.
//!
//! Reemplaza el modelo "todo o nada" de Unix (root/user) y el ACL de
//! Windows con capacidades finas. Cada `BmoHandle` tiene un conjunto de
//! capacidades que determinan qué operaciones se permiten.
//!
//! `BmoStatus::NEEDS_CAP` se devuelve cuando falta una capacidad.

use crate::bmo_abi::primitives::bx_u64;

/// Una capacidad individual — 64 bits que identifican un permiso único.
///
/// Diseñado para usarse como flag en un bitset de 64 capacidades.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoCap(pub bx_u64);
const _BMO_CAP_SIZE: () = assert!(core::mem::size_of::<BmoCap>() == 8);

impl BmoCap {
    pub const fn new(id: bx_u64) -> Self {
        Self(id)
    }

    pub const fn as_u64(&self) -> bx_u64 {
        self.0
    }
}

/// Conjunto de hasta 64 capacidades (bitset).
///
/// # Layout (8 bytes)
/// ```text
/// [0..7] bits: u64 — cada bit 1 = capacidad presente
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoCapSet {
    bits: bx_u64,
}
const _: () = assert!(core::mem::size_of::<BmoCapSet>() == 8);

impl BmoCapSet {
    pub const EMPTY: Self = Self { bits: 0 };
    pub const ALL: Self = Self { bits: !0 };

    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    pub const fn has(&self, cap: BmoCap) -> bool {
        // Cap ids are 0..63; an out-of-range id is never present instead
        // of triggering a shift-overflow panic/UB.
        cap.0 < 64 && (self.bits & (1 << cap.0)) != 0
    }

    pub fn add(&mut self, cap: BmoCap) {
        if cap.0 < 64 {
            self.bits |= 1 << cap.0;
        }
    }

    pub fn remove(&mut self, cap: BmoCap) {
        if cap.0 < 64 {
            self.bits &= !(1 << cap.0);
        }
    }

    pub const fn contains(&self, other: &BmoCapSet) -> bool {
        (self.bits & other.bits) == other.bits
    }

    pub const fn bits(&self) -> bx_u64 {
        self.bits
    }
}

// ─── System capabilities (0..15 reserved) ─────────────────────────

impl BmoCap {
    /// Leer el contenido de un handle.
    pub const READ: Self = Self(0);
    /// Escribir/modificar un handle.
    pub const WRITE: Self = Self(1);
    /// Ejecutar (para archivos/procesos).
    pub const EXEC: Self = Self(2);
    /// Crear nuevos objetos/handles.
    pub const CREATE: Self = Self(3);
    /// Eliminar/destruir objetos.
    pub const DELETE: Self = Self(4);
    /// Enumerar contenido (directorios, puertos).
    pub const ENUMERATE: Self = Self(5);
    /// Esperar a que un objeto esté listo (sincronización).
    pub const WAIT: Self = Self(6);
    /// Heredar capacidades a hijos.
    pub const INHERIT: Self = Self(7);
    /// Elevar privilegios (requiere validación adicional).
    pub const ELEVATE: Self = Self(8);
    /// Acceso a dispositivos físicos (Ring 0).
    pub const DEVICE: Self = Self(9);
    /// Acceso a red (sockets).
    pub const NETWORK: Self = Self(10);
    /// Acceso a ventanas / display.
    pub const DISPLAY: Self = Self(11);
    /// Gestión de memoria física.
    pub const MEMORY: Self = Self(12);
    /// Gestión de procesos/hilos.
    pub const PROCESS: Self = Self(13);
    /// Depuración de otros procesos.
    pub const DEBUG: Self = Self(14);
}
