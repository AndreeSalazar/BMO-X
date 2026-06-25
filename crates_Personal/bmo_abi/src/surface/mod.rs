//! `bmo_abi::surface` — Superficies CPU/GPU.
//!
//! Una surface es un buffer de píxeles con un **formato** conocido.
//! Puede vivir en:
//! - **CPU memory** (accesible por el kernel con `memcpy`).
//! - **GPU memory** (DMA-backed, requiere flush/invalidate).
//!
//! ## Syscalls (ver `syscalls/mod.rs`)
//!
//! - `NR_SURFACE_MAP`    (0x1C0) → `bmo_surface_map(info) -> BmoSurface`
//! - `NR_SURFACE_UNMAP`  (0x1C1) → `bmo_surface_unmap(s)`
//! - `NR_SURFACE_PRESENT` (0x1C2) → `bmo_surface_present(s, dst_window)`

#![allow(dead_code)]

use crate::bmo_abi::fundamentals::handle::BmoHandle;

// ─── Pixel format ──────────────────────────────────────────────────

/// Formato de pixel. Ver `BmoFormat::*` para el layout exacto.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoFormat {
    /// Desconocido / inválido.
    Unknown   = 0,
    /// 8-bit rojo (1 byte/pixel). Usado para masks/alpha.
    R8        = 1,
    /// 8-bit alpha.
    A8        = 2,
    /// 16-bit RG (2 bytes/pixel). Poco común.
    RG8       = 3,
    /// 24-bit RGB (3 bytes/pixel, padded a 4 en memoria).
    RGB24     = 4,
    /// 32-bit RGBA. Cada pixel 4 bytes: R, G, B, A.
    RGBA8     = 5,
    /// 32-bit BGRA. Cada pixel 4 bytes: B, G, R, A.
    BGRA8     = 6,
    /// 32-bit ARGB (Windows style).
    ARGB8     = 7,
    /// 32-bit ABGR.
    ABGR8     = 8,
}

impl BmoFormat {
    /// Bytes por pixel.
    #[inline]
    pub fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::Unknown | Self::R8 | Self::A8 => 1,
            Self::RG8 => 2,
            Self::RGB24 => 3,
            Self::RGBA8 | Self::BGRA8 | Self::ARGB8 | Self::ABGR8 => 4,
        }
    }

    /// `true` si el formato tiene canal alpha.
    #[inline]
    pub fn has_alpha(self) -> bool {
        matches!(self, Self::A8 | Self::RGBA8 | Self::BGRA8 | Self::ARGB8 | Self::ABGR8)
    }
}

// ─── Surface flags ─────────────────────────────────────────────────

/// Dónde vive el buffer de la surface.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoSurfaceKind {
    /// Memoria CPU (accesible directamente).
    Cpu  = 0,
    /// Memoria GPU (DMA-backed).
    Gpu  = 1,
    /// Lazy: la surface se computa on-demand (shader, blur, etc.).
    Lazy = 2,
}

/// Cómo se presenta la surface.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoPresentMode {
    /// Copy: el kernel copia la surface a la ventana.
    Copy   = 0,
    /// Blit: el kernel blitea con alpha.
    Blit   = 1,
    /// Flip: el kernel hace page-flip (efficient, requiere GPU).
    Flip   = 2,
}

/// Info para crear una surface.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BmoSurfaceInfo {
    pub w: u32,
    pub h: u32,
    pub format: BmoFormat,
    pub kind: BmoSurfaceKind,
    /// Si es CPU, hint de alineación del buffer.
    pub align: u32,
}

impl BmoSurfaceInfo {
    /// Tamaño en bytes del buffer de la surface.
    #[inline]
    pub fn size_bytes(&self) -> u32 {
        self.w * self.h * self.format.bytes_per_pixel()
    }
}

/// Handle a una surface. Proceso-local.
pub type BmoSurface = BmoHandle;
