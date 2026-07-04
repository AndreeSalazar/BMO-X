//! `defense::capability` — Capabilities por proceso.

#![allow(dead_code)]

/// Una capability individual.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    /// Acceso a archivos.
    FileAccess,
    /// Acceso a red.
    Network,
    /// Crear procesos hijos.
    Spawn,
    /// Allocación de memoria.
    MemAlloc,
    /// Acceder a Ring 0 (debug only).
    Ring0,
    /// Manipular ventanas.
    Windowing,
    /// Llamar syscalls de audio.
    Audio,
    /// Llamar syscalls de GPU.
    Gpu,
}

/// Conjunto de capabilities (bitfield-like).
#[derive(Clone, Copy, Default, Debug)]
pub struct CapabilitySet(pub u32);

impl CapabilitySet {
    pub const fn empty() -> Self { Self(0) }
    pub const fn all() -> Self { Self(0xFFFF_FFFF) }

    pub fn has(self, c: Capability) -> bool {
        let bit = cap_bit(c);
        (self.0 & bit) != 0
    }
    pub fn grant(&mut self, c: Capability) { self.0 |= cap_bit(c); }
    pub fn revoke(&mut self, c: Capability) { self.0 &= !cap_bit(c); }
}

const fn cap_bit(c: Capability) -> u32 {
    match c {
        Capability::FileAccess => 1 << 0,
        Capability::Network    => 1 << 1,
        Capability::Spawn      => 1 << 2,
        Capability::MemAlloc   => 1 << 3,
        Capability::Ring0      => 1 << 4,
        Capability::Windowing  => 1 << 5,
        Capability::Audio      => 1 << 6,
        Capability::Gpu        => 1 << 7,
    }
}

pub fn init() {
    // v1.8.8: capabilities globales = todas concedidas (sin sandbox).
}

/// Extrae las capabilities requeridas por un BEF (vía header de imports).
pub fn required_from_bef(bytes: &[u8]) -> alloc::vec::Vec<Capability> {
    let _ = bytes;
    // v1.8.8: stub. En v1.9, parsear la sección de imports.
    alloc::vec::Vec::new()
}
