//! `bmo_abi::drawing` — Primitivas de dibujo 2D.
//!
//! Define los **tipos** que las funciones `draw_*` y `win_draw_*`
//! (declaradas en `crate::bmo_abi::syscalls`) reciben como argumentos.
//!
//! ## Modelo de color
//!
//! BMO usa **RGBA8** como formato canónico de pixel. El byte más
//! significativo es **R**, el menos significativo es **A**:
//!
//! ```text
//! 0xAARRGGBB
//! ```
//!
//! El endianness de la CPU no importa: las funciones `r()`, `g()`,
//! `b()`, `a()` extraen los bytes en el orden correcto.

#![allow(dead_code)]

use crate::bmo_abi::fundamentals::handle::BmoHandle;

// ─── Color ──────────────────────────────────────────────────────────

/// Color RGBA8 empaquetado en `u32`. Ver módulo-level docs para el layout.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct BmoColor(pub u32);

impl BmoColor {
    /// Negro opaco.
    pub const BLACK:   Self = Self(0xFF00_0000);
    /// Blanco opaco.
    pub const WHITE:   Self = Self(0xFFFF_FFFF);
    /// Rojo opaco.
    pub const RED:     Self = Self(0xFF00_00FF);
    /// Verde opaco.
    pub const GREEN:   Self = Self(0xFF00_FF00);
    /// Azul opaco.
    pub const BLUE:    Self = Self(0xFFFF_0000);
    /// Transparente (alpha = 0).
    pub const TRANSPARENT: Self = Self(0x0000_0000);

    /// Construye desde componentes R, G, B, A (0..=255).
    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self(
            ((a as u32) << 24)
            | ((r as u32) << 16)
            | ((g as u32) << 8)
            | (b as u32)
        )
    }

    /// Construye desde RGB, alpha = 255.
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    #[inline] pub const fn r(self) -> u8 { ((self.0 >> 16) & 0xFF) as u8 }
    #[inline] pub const fn g(self) -> u8 { ((self.0 >>  8) & 0xFF) as u8 }
    #[inline] pub const fn b(self) -> u8 { ((self.0      ) & 0xFF) as u8 }
    #[inline] pub const fn a(self) -> u8 { ((self.0 >> 24) & 0xFF) as u8 }

    /// Convierte a formato BGRA8 (útil para algunos blits).
    #[inline]
    pub const fn to_bgra(self) -> u32 {
        ((self.0 & 0xFF00_0000))
        | ((self.0 & 0x00FF_0000) >> 16)
        | ((self.0 & 0x0000_FF00))
        | ((self.0 & 0x0000_00FF) << 16)
    }

    /// Alpha-blend sobre otro color (resultado = self sobre dst).
    /// Fórmula: `out = src * a + dst * (1 - a)`.
    #[inline]
    pub fn blend_over(self, dst: Self) -> Self {
        let sa = self.a() as u32;
        let da = 255 - sa;
        let r = (self.r() as u32 * sa + dst.r() as u32 * da) / 255;
        let g = (self.g() as u32 * sa + dst.g() as u32 * da) / 255;
        let b = (self.b() as u32 * sa + dst.b() as u32 * da) / 255;
        Self::rgba(r as u8, g as u8, b as u8, 255)
    }
}

// ─── Point ──────────────────────────────────────────────────────────

/// Punto 2D en coordenadas enteras (pixel coords).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BmoPoint {
    pub x: i32,
    pub y: i32,
}

impl BmoPoint {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub const fn new(x: i32, y: i32) -> Self { Self { x, y } }
}

// ─── Rect ───────────────────────────────────────────────────────────

/// Rectángulo con esquinas enteras.
///
/// **Semántica de inclusión**: `[x, x+w) × [y, y+h)`.
/// Un rect de w=0 o h=0 es **vacío**.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BmoRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl BmoRect {
    pub const EMPTY: Self = Self { x: 0, y: 0, w: 0, h: 0 };

    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self { Self { x, y, w, h } }

    #[inline] pub const fn left(&self)   -> i32 { self.x }
    #[inline] pub const fn top(&self)    -> i32 { self.y }
    #[inline] pub const fn right(&self)  -> i32 { self.x + self.w }
    #[inline] pub const fn bottom(&self) -> i32 { self.y + self.h }
    #[inline] pub const fn is_empty(&self) -> bool { self.w <= 0 || self.h <= 0 }

    /// Área en píxeles.
    #[inline]
    pub const fn area(&self) -> i64 { self.w as i64 * self.h as i64 }

    /// `true` si el punto está dentro (semántica half-open).
    #[inline]
    pub fn contains(&self, p: BmoPoint) -> bool {
        p.x >= self.x && p.x < self.right()
        && p.y >= self.y && p.y < self.bottom()
    }

    /// Intersección de dos rects. Retorna `EMPTY` si no intersectan.
    pub fn intersect(&self, other: &BmoRect) -> BmoRect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = self.right().min(other.right());
        let b = self.bottom().min(other.bottom());
        if r <= x || b <= y { Self::EMPTY } else { Self::new(x, y, r - x, b - y) }
    }
}

// ─── Font ───────────────────────────────────────────────────────────

/// Handle a una fuente cargada.
pub type BmoFontHandle = BmoHandle;

/// Estilo de fuente.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoFontStyle {
    Regular  = 0,
    Bold     = 1,
    Italic   = 2,
    BoldItalic = 3,
    Monospace = 4,
}

// ─── Draw flags ─────────────────────────────────────────────────────

/// Modo de dibujo para shapes.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BmoDrawMode {
    /// Rellenar el interior.
    Fill   = 0,
    /// Solo el borde.
    Stroke = 1,
    /// Rellenar + borde.
    FillStroke = 2,
}

/// Flags para `bmo_draw_blit` / `bmo_draw_text`.
#[derive(Clone, Copy, Debug, Default)]
pub struct BmoBlitFlags(pub u32);

impl BmoBlitFlags {
    pub const NONE:     Self = Self(0);
    /// Aplicar alpha blending.
    pub const BLEND:    Self = Self(1 << 0);
    /// Mantener alpha del source.
    pub const KEEP_ALPHA: Self = Self(1 << 1);
    /// Repetir (tiling) en lugar de stretch.
    pub const TILE:     Self = Self(1 << 2);
    /// Volteo horizontal.
    pub const FLIP_H:   Self = Self(1 << 3);
    /// Volteo vertical.
    pub const FLIP_V:   Self = Self(1 << 4);

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}
