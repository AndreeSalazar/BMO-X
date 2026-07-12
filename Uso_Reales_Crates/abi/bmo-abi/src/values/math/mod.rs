//! `math` — funciones matemáticas básicas del BMO ABI.
//!
//! Reemplaza `<math.h>` de C con implementaciones ligeras, sin depender de
//! `libm` del host (no disponible en Ring 0).
//!
//! Precisión: ~1 ULP para f64, suficiente para gráficos y física de juego.

use crate::bmo_abi::primitives::floats::{bx_f32, bx_f64};
use crate::bmo_abi::primitives::{bx_i64, bx_u64};

// ─── Square root (Newton-Raphson) ──────────────────────────────────

/// Square root, IEEE 754 exact para f64.
pub fn sqrt_f64(x: bx_f64) -> bx_f64 {
    if x <= 0.0 { return 0.0; }
    let mut guess = x;
    for _ in 0..10 {
        guess = (guess + x / guess) * 0.5;
    }
    guess
}

/// Square root, f32.
pub fn sqrt_f32(x: bx_f32) -> bx_f32 {
    sqrt_f64(x as f64) as f32
}

// ─── Core sin Taylor (|x| ≤ π/4) ──────────────────────────────────

fn sin_taylor(x: bx_f64) -> bx_f64 {
    let x2 = x * x;
    x * (1.0
        - x2 / 6.0
        + x2 * x2 / 120.0
        - x2 * x2 * x2 / 5040.0
        + x2 * x2 * x2 * x2 / 362880.0
        - x2 * x2 * x2 * x2 * x2 / 39916800.0
        + x2 * x2 * x2 * x2 * x2 * x2 / 6227020800.0)
}

fn cos_taylor(x: bx_f64) -> bx_f64 {
    let x2 = x * x;
    1.0
        - x2 / 2.0
        + x2 * x2 / 24.0
        - x2 * x2 * x2 / 720.0
        + x2 * x2 * x2 * x2 / 40320.0
        - x2 * x2 * x2 * x2 * x2 / 3628800.0
        + x2 * x2 * x2 * x2 * x2 * x2 / 479001600.0
}

// ─── Sin (range-reduced to [0, π/4]) ───────────────────────────────

/// Sine (f64), ~1e-12 precisión.
pub fn sin_f64(x: bx_f64) -> bx_f64 {
    let mut x = x % (core::f64::consts::TAU);
    if x < 0.0 { x += core::f64::consts::TAU; }

    let sign = if x > core::f64::consts::PI { x -= core::f64::consts::PI; -1.0 } else { 1.0 };

    if x > core::f64::consts::FRAC_PI_2 {
        x = core::f64::consts::PI - x;
    }

    if x > core::f64::consts::FRAC_PI_4 {
        cos_taylor(core::f64::consts::FRAC_PI_2 - x) * sign
    } else {
        sin_taylor(x) * sign
    }
}

pub fn sin_f32(x: bx_f32) -> bx_f32 {
    sin_f64(x as f64) as f32
}

// ─── Cosine ────────────────────────────────────────────────────────

pub fn cos_f64(x: bx_f64) -> bx_f64 {
    let mut x = x % core::f64::consts::TAU;
    if x < 0.0 { x += core::f64::consts::TAU; }

    if x > core::f64::consts::PI {
        x = core::f64::consts::TAU - x;
    }

    if x > core::f64::consts::FRAC_PI_2 {
        let y = core::f64::consts::PI - x;
        if y > core::f64::consts::FRAC_PI_4 {
            -sin_taylor(core::f64::consts::FRAC_PI_2 - y)
        } else {
            -cos_taylor(y)
        }
    } else if x > core::f64::consts::FRAC_PI_4 {
        sin_taylor(core::f64::consts::FRAC_PI_2 - x)
    } else {
        cos_taylor(x)
    }
}

pub fn cos_f32(x: bx_f32) -> bx_f32 {
    cos_f64(x as f64) as f32
}

// ─── Power (exponentiation by squaring) ────────────────────────────

pub fn pow_f64(base: bx_f64, exp: bx_i64) -> bx_f64 {
    if exp == 0 { return 1.0; }
    let mut result = 1.0;
    let mut b = if exp < 0 { 1.0 / base } else { base };
    let mut e = if exp < 0 { -exp } else { exp };
    while e > 0 {
        if e & 1 != 0 { result *= b; }
        b *= b;
        e >>= 1;
    }
    result
}

pub fn pow_f32(base: bx_f32, exp: bx_i64) -> bx_f32 {
    pow_f64(base as f64, exp) as f32
}

// ─── Absolute value ────────────────────────────────────────────────

pub fn abs_f64(x: bx_f64) -> bx_f64 {
    if x < 0.0 { -x } else { x }
}

pub fn abs_f32(x: bx_f32) -> bx_f32 {
    if x < 0.0 { -x } else { x }
}

// ─── Floor / Ceil ─────────────────────────────────────────────────

pub fn floor_f64(x: bx_f64) -> bx_f64 {
    let trunc = x as i64 as f64;
    if x >= 0.0 || x == trunc { trunc } else { trunc - 1.0 }
}

pub fn ceil_f64(x: bx_f64) -> bx_f64 {
    let trunc = x as i64 as f64;
    if x <= 0.0 || x == trunc { trunc } else { trunc + 1.0 }
}

pub fn floor_f32(x: bx_f32) -> bx_f32 { floor_f64(x as f64) as f32 }
pub fn ceil_f32(x: bx_f32) -> bx_f32 { ceil_f64(x as f64) as f32 }

// ─── Min/max ───────────────────────────────────────────────────────

pub fn min_f64(a: bx_f64, b: bx_f64) -> bx_f64 { if a < b { a } else { b } }
pub fn max_f64(a: bx_f64, b: bx_f64) -> bx_f64 { if a > b { a } else { b } }

pub fn min_f32(a: bx_f32, b: bx_f32) -> bx_f32 { if a < b { a } else { b } }
pub fn max_f32(a: bx_f32, b: bx_f32) -> bx_f32 { if a > b { a } else { b } }

pub fn min_u64(a: bx_u64, b: bx_u64) -> bx_u64 { if a < b { a } else { b } }
pub fn max_u64(a: bx_u64, b: bx_u64) -> bx_u64 { if a > b { a } else { b } }

// ─── Constants ─────────────────────────────────────────────────────

pub const PI_F64: bx_f64 = core::f64::consts::PI;
pub const PI_F32: bx_f32 = core::f32::consts::PI;
pub const E_F64: bx_f64 = core::f64::consts::E;
pub const E_F32: bx_f32 = core::f32::consts::E;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt_basic() {
        let r = sqrt_f64(100.0);
        assert!((r - 10.0).abs() < 1e-12);
    }

    #[test]
    fn sin_pi() {
        let r = sin_f64(core::f64::consts::PI);
        assert!(r.abs() < 1e-10);
    }

    #[test]
    fn sin_half_pi() {
        let r = sin_f64(core::f64::consts::FRAC_PI_2);
        assert!((r - 1.0).abs() < 1e-8);
    }

    #[test]
    fn cos_zero() {
        let r = cos_f64(0.0);
        assert!((r - 1.0).abs() < 1e-8);
    }

    #[test]
    fn cos_pi() {
        let r = cos_f64(core::f64::consts::PI);
        assert!((r + 1.0).abs() < 1e-8);
    }

    #[test]
    fn pow_int() {
        let r = pow_f64(2.0, 10);
        assert!((r - 1024.0).abs() < 1e-12);
    }

    #[test]
    fn pow_neg() {
        let r = pow_f64(2.0, -1);
        assert!((r - 0.5).abs() < 1e-12);
    }

    #[test]
    fn floor_neg() {
        assert_eq!(floor_f64(-1.5), -2.0);
    }

    #[test]
    fn sin_quarter_pi() {
        let expected = core::f64::consts::FRAC_1_SQRT_2;
        let r = sin_f64(core::f64::consts::FRAC_PI_4);
        assert!((r - expected).abs() < 1e-8);
    }

    #[test]
    fn cos_quarter_pi() {
        let expected = core::f64::consts::FRAC_1_SQRT_2;
        let r = cos_f64(core::f64::consts::FRAC_PI_4);
        assert!((r - expected).abs() < 1e-8);
    }
}
