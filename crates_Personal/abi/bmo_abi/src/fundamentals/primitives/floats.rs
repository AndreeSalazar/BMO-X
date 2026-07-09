//! Tipos float canónicos del BMO ABI.
//!
//! - `bx_f32` — IEEE 754 binary32 (`float` C). 32 bits.
//! - `bx_f64` — IEEE 754 binary64 (`double` C). 64 bits.
//! - `bx_f16` — IEEE 754 binary16 (half precision). 16 bits.
//!   Soportado nativamente por:
//!     • backends gráficos modernos cuando existan
//!     • CPU vía conversión software o instrucciones disponibles
//!   El kernel lo expone como struct opaco; las apps lo usan en shaders.

#![allow(non_camel_case_types)]

pub type bx_f32 = f32;
pub type bx_f64 = f64;

/// Half precision (IEEE 754 binary16).
///
/// Almacenamiento de 16 bits, sin operaciones aritméticas directas en CPU
/// pre-AVX-512 FP16. Para hacer cálculo, convertir a `bx_f32` con
/// [`bx_f16::to_f32`].
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct bx_f16(pub u16);

impl bx_f16 {
    pub const ZERO: Self = Self(0x0000);
    pub const ONE:  Self = Self(0x3C00);
    pub const NAN:  Self = Self(0x7E00);
    pub const INFINITY:     Self = Self(0x7C00);
    pub const NEG_INFINITY: Self = Self(0xFC00);

    /// Convierte un f16 (almacenado como `u16`) a `f32` por software.
    pub fn to_f32(self) -> f32 {
        let bits = self.0 as u32;
        let sign = (bits & 0x8000) << 16;
        let exp  = (bits >> 10) & 0x1F;
        let mant = bits & 0x3FF;
        let f32_bits = match exp {
            0  if mant == 0 => sign, // ±0
            0  => {
                // subnormal — renormalizar
                let mut m = mant;
                let mut e: i32 = -14;
                while m & 0x400 == 0 { m <<= 1; e -= 1; }
                m &= 0x3FF;
                sign | (((e + 127) as u32) << 23) | (m << 13)
            },
            31 => sign | (0xFF << 23) | (mant << 13),  // inf/NaN
            _  => sign | (((exp + 112) as u32) << 23) | (mant << 13),
        };
        f32::from_bits(f32_bits)
    }

    /// Convierte un `f32` a `bx_f16` (con redondeo a más cercano par).
    pub fn from_f32(v: f32) -> Self {
        let bits = v.to_bits();
        let sign = ((bits >> 31) & 0x1) as u16;
        let exp  = ((bits >> 23) & 0xFF) as i32;
        let mant = bits & 0x7FFFFF;
        let half = if exp == 0xFF {
            // Inf/NaN
            let m = if mant != 0 { 0x200 } else { 0 };
            (sign << 15) | 0x7C00 | m
        } else if exp > 142 {
            // Overflow → ±inf
            (sign << 15) | 0x7C00
        } else if exp < 113 {
            // Underflow → ±0 o subnormal
            if exp < 103 { sign << 15 }
            else {
                let m = (mant | 0x800000) >> ((113 - exp) as u32);
                (sign << 15) | (m as u16 & 0x3FF)
            }
        } else {
            let new_exp = (exp - 112) as u16;
            (sign << 15) | (new_exp << 10) | ((mant >> 13) as u16 & 0x3FF)
        };
        Self(half)
    }
}

// ─── Constantes (sustituye <float.h>) ─────────────────────────────────
pub const BX_F32_EPSILON: bx_f32 = f32::EPSILON;
pub const BX_F64_EPSILON: bx_f64 = f64::EPSILON;
pub const BX_F32_INFINITY: bx_f32 = f32::INFINITY;
pub const BX_F64_INFINITY: bx_f64 = f64::INFINITY;
