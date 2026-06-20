//! v2.0 — Tabla de superficies (offscreen buffers para doble buffer).
//!
//! Cada ventana tiene una surface; las superficies también pueden ser
//! compartidas entre procesos (v2.0: 1:1 ventana↔surface).

#![allow(dead_code)]

pub const MAX_SURFACES: usize = 256;
pub const SURFACE_INVALID: u32 = 0xFFFFFFFF;

pub mod format {
    pub const ARGB32: u32 = 0x01;
    pub const XRGB32: u32 = 0x02;
}

#[derive(Debug, Clone, Copy)]
pub struct BmoSurface {
    pub used: bool,
    pub id: u32,
    pub generation: u16,
    pub width: u16,
    pub height: u16,
    pub pitch: u16,        // bytes por fila (múltiplo de 16)
    pub format: u32,
    pub phys_addr: u64,    // dirección física (para DMA futuro)
    pub virt_addr: u64,    // dirección virtual mapeada a Ring 3
    pub refcount: u32,
    pub owner_window: u32,
    pub flags: u16,        // SF_DIRTY, SF_LOCKED
}

impl BmoSurface {
    pub const fn empty() -> Self {
        Self {
            used: false, id: 0, generation: 0,
            width: 0, height: 0, pitch: 0,
            format: format::XRGB32,
            phys_addr: 0, virt_addr: 0,
            refcount: 0, owner_window: SURFACE_INVALID,
            flags: 0,
        }
    }
}

pub struct SurfaceTable {
    pub surfaces: [BmoSurface; MAX_SURFACES],
    pub next_id: u32,
}

impl SurfaceTable {
    pub const fn new() -> Self {
        const S: BmoSurface = BmoSurface::empty();
        Self {
            surfaces: [S; MAX_SURFACES],
            next_id: 1,
        }
    }

    pub fn init(&mut self) {
        for s in self.surfaces.iter_mut() { *s = BmoSurface::empty(); }
        self.next_id = 1;
    }

    /// Reserva una surface del tamaño pedido. Asigna un buffer en el
    /// heap del kernel (v2.0: usa un buffer estático interno).
    pub fn alloc(&mut self, w: u16, h: u16, format: u32, owner: u32) -> Option<u32> {
        for (i, s) in self.surfaces.iter_mut().enumerate() {
            if !s.used {
                s.used = true;
                s.id = self.next_id;
                s.generation = s.generation.wrapping_add(1);
                s.width = w;
                s.height = h;
                s.pitch = ((w as u32) * 4).next_multiple_of(16) as u16;
                s.format = format;
                s.refcount = 1;
                s.owner_window = owner;
                // virt_addr se asigna fuera (en syscall.rs, que tiene
                // acceso al kernel heap). Aquí dejamos 0 como sentinel.
                s.virt_addr = 0;
                s.phys_addr = 0;
                s.flags = 0;
                self.next_id = self.next_id.wrapping_add(1);
                return Some(i as u32);
            }
        }
        None
    }

    pub fn free(&mut self, slot: u32) -> bool {
        if let Some(s) = self.surfaces.get_mut(slot as usize) {
            if !s.used { return false; }
            s.used = false;
            s.generation = s.generation.wrapping_add(1);
            s.refcount = 0;
            s.virt_addr = 0;
            true
        } else { false }
    }

    pub fn surface(&self, slot: u32) -> Option<&BmoSurface> {
        self.surfaces.get(slot as usize).and_then(|s| if s.used { Some(s) } else { None })
    }
    pub fn surface_mut(&mut self, slot: u32) -> Option<&mut BmoSurface> {
        self.surfaces.get_mut(slot as usize).and_then(|s| if s.used { Some(s) } else { None })
    }
}

/// Backing storage estático para surfaces. Tamaño total ~ 32 MB a
/// 1920×1080×4 bytes por surface — por eso limitamos a 256 y a 1080p.
/// En una build real esto se asigna dinámicamente desde el kernel heap.
pub const MAX_SURFACE_BYTES: usize = 1920 * 1080 * 4;
static mut SURFACE_STORAGE: [u32; MAX_SURFACE_BYTES / 4] = [0; MAX_SURFACE_BYTES / 4];

/// Devuelve la dirección virtual del backing storage de una surface.
/// En v2.0 todas comparten el mismo buffer estático (no se hace
/// compositing real) — se usa solo para validación de punteros.
pub fn surface_storage_addr() -> u64 {
    unsafe { SURFACE_STORAGE.as_ptr() as u64 }
}
