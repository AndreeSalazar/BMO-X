//! v2.0 — Tabla de superficies (offscreen buffers para doble buffer).
//!
//! Cada ventana tiene una surface; las superficies también pueden ser
//! compartidas entre procesos. Cada surface obtiene su propio buffer
//! de pixels desde un pool estático.

#![allow(dead_code)]

use core::sync::atomic::{AtomicU8, Ordering};

pub const MAX_SURFACES: usize = 64;
pub const SURFACE_INVALID: u32 = 0xFFFFFFFF;

pub mod format {
    pub const ARGB32: u32 = 0x01;
    pub const XRGB32: u32 = 0x02;
}

/// Tamaño máximo por surface: 1920×1080×4 = 8.294.400 bytes.
const MAX_SURFACE_PIXELS: usize = 1920 * 1080;
/// Pool estático de pixels: 4 surfaces × 8 MB = 32 MB.
const SURFACE_POOL_SLOTS: usize = 4;

#[derive(Debug, Clone, Copy)]
pub struct BmoSurface {
    pub used: bool,
    pub id: u32,
    pub generation: u16,
    pub width: u16,
    pub height: u16,
    pub pitch: u16,
    pub format: u32,
    pub pixels: *mut u32,
    pub phys_addr: u64,
    pub refcount: u32,
    pub owner_window: u32,
    pub flags: u16,
}

impl BmoSurface {
    pub const fn empty() -> Self {
        Self {
            used: false, id: 0, generation: 0,
            width: 0, height: 0, pitch: 0,
            format: format::XRGB32,
            pixels: core::ptr::null_mut(),
            phys_addr: 0,
            refcount: 0, owner_window: SURFACE_INVALID,
            flags: 0,
        }
    }
}

static mut POOL_BUFFERS: [[u32; MAX_SURFACE_PIXELS]; SURFACE_POOL_SLOTS] =
    [[0u32; MAX_SURFACE_PIXELS]; SURFACE_POOL_SLOTS];
static mut POOL_USED: [bool; SURFACE_POOL_SLOTS] = [false; SURFACE_POOL_SLOTS];

unsafe fn pool_alloc() -> Option<*mut u32> {
    for (i, used) in POOL_USED.iter_mut().enumerate() {
        if !*used {
            *used = true;
            return Some(POOL_BUFFERS[i].as_mut_ptr());
        }
    }
    None
}

unsafe fn pool_free(ptr: *mut u32) {
    for (i, buf) in POOL_BUFFERS.iter().enumerate() {
        if buf.as_ptr() == ptr {
            POOL_USED[i] = false;
            POOL_BUFFERS[i] = [0u32; MAX_SURFACE_PIXELS];
            return;
        }
    }
}

pub struct SurfaceTable {
    pub surfaces: [BmoSurface; MAX_SURFACES],
    pub next_id: u32,
    lock: AtomicU8,
}

impl SurfaceTable {
    pub const fn new() -> Self {
        const S: BmoSurface = BmoSurface::empty();
        Self {
            surfaces: [S; MAX_SURFACES],
            next_id: 1,
            lock: AtomicU8::new(0),
        }
    }

    pub fn acquire(&self) {
        loop {
            match self.lock.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => return,
                Err(_) => core::hint::spin_loop(),
            }
        }
    }
    pub fn release(&self) { self.lock.store(0, Ordering::Release); }

    pub fn init(&mut self) {
        for s in self.surfaces.iter_mut() { *s = BmoSurface::empty(); }
        self.next_id = 1;
        unsafe {
            POOL_USED = [false; SURFACE_POOL_SLOTS];
        }
    }

    pub fn alloc(&mut self, w: u16, h: u16, format: u32, owner: u32) -> Option<u32> {
        let pixels = unsafe { pool_alloc() };
        let pixels = match pixels {
            Some(p) => p,
            None => return None,
        };
        for (i, s) in self.surfaces.iter_mut().enumerate() {
            if !s.used {
                s.used = true;
                s.id = self.next_id;
                s.generation = s.generation.wrapping_add(1);
                s.width = w;
                s.height = h;
                s.pitch = ((w as u32) * 4).next_multiple_of(16) as u16;
                s.format = format;
                s.pixels = pixels;
                s.phys_addr = pixels as u64;
                s.refcount = 1;
                s.owner_window = owner;
                s.flags = 0;
                self.next_id = self.next_id.wrapping_add(1);
                return Some(i as u32);
            }
        }
        unsafe { pool_free(pixels); }
        None
    }

    pub fn free(&mut self, slot: u32) -> bool {
        if let Some(s) = self.surfaces.get_mut(slot as usize) {
            if !s.used { return false; }
            if !s.pixels.is_null() {
                unsafe { pool_free(s.pixels); }
            }
            s.used = false;
            s.generation = s.generation.wrapping_add(1);
            s.refcount = 0;
            s.pixels = core::ptr::null_mut();
            true
        } else { false }
    }

    pub fn surface(&self, slot: u32) -> Option<&BmoSurface> {
        self.surfaces.get(slot as usize).and_then(|s| if s.used { Some(s) } else { None })
    }
    pub fn surface_mut(&mut self, slot: u32) -> Option<&mut BmoSurface> {
        self.surfaces.get_mut(slot as usize).and_then(|s| if s.used { Some(s) } else { None })
    }

    pub fn pixels_for(&self, slot: u32) -> Option<*mut u32> {
        self.surface(slot).map(|s| s.pixels)
    }

    pub fn size_for(&self, slot: u32) -> Option<(u16, u16, u16)> {
        self.surface(slot).map(|s| (s.width, s.height, s.pitch))
    }
}

/// Devuelve el puntero base de un surface slot (o null).
pub fn surface_pixels(slot: u32) -> *mut u32 {
    unsafe {
        let st = &SURFACE_TABLE;
        st.surface(slot).map(|s| s.pixels).unwrap_or(core::ptr::null_mut())
    }
}

pub fn surface_storage_addr() -> u64 {
    unsafe {
        let st = &SURFACE_TABLE;
        st.surfaces.iter().find(|s| s.used).map(|s| s.pixels as u64).unwrap_or(0)
    }
}

static mut SURFACE_TABLE: SurfaceTable = SurfaceTable::new();

pub fn surface_table() -> &'static mut SurfaceTable {
    unsafe { &mut SURFACE_TABLE }
}
