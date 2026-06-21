//! `math` — Funciones numéricas del BMO ABI.
//!
//! Reemplaza `<math.h>` de C con implementaciones pure-Rust (no libm).
//! Precisión: suficiente para gráficos y juegos (no certificación IEEE).

#![allow(dead_code)]

pub fn abs_f64(x: f64) -> f64 {
    if x < 0.0 { -x } else { x }
}

pub fn abs_i64(x: i64) -> i64 {
    if x < 0 { -x } else { x }
}

pub fn min_f64(a: f64, b: f64) -> f64 {
    if a < b { a } else { b }
}

pub fn max_f64(a: f64, b: f64) -> f64 {
    if a > b { a } else { b }
}

pub fn clamp_f64(v: f64, lo: f64, hi: f64) -> f64 {
    min_f64(max_f64(v, lo), hi)
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

pub fn sqrt_f64(x: f64) -> f64 {
    if x < 0.0 { return 0.0; }
    if x == 0.0 { return 0.0; }
    let mut guess = x;
    let mut i = 0;
    while i < 50 {
        guess = (guess + x / guess) / 2.0;
        i += 1;
    }
    guess
}

pub fn pow_f64(base: f64, exp: i32) -> f64 {
    if exp == 0 { return 1.0; }
    let mut result = 1.0;
    let mut b = base;
    let mut e = exp.unsigned_abs();
    while e > 0 {
        if e & 1 == 1 {
            result *= b;
        }
        b *= b;
        e >>= 1;
    }
    if exp < 0 { 1.0 / result } else { result }
}

pub fn sin_f64(x: f64) -> f64 {
    let mut x = x;
    while x > core::f64::consts::PI { x -= 2.0 * core::f64::consts::PI; }
    while x < -core::f64::consts::PI { x += 2.0 * core::f64::consts::PI; }
    let x2 = x * x;
    let x3 = x2 * x;
    let x5 = x3 * x2;
    let x7 = x5 * x2;
    let x9 = x7 * x2;
    x - x3 / 6.0 + x5 / 120.0 - x7 / 5040.0 + x9 / 362880.0
}

pub fn cos_f64(x: f64) -> f64 {
    sin_f64(x + core::f64::consts::FRAC_PI_2)
}

pub fn tan_f64(x: f64) -> f64 {
    let c = cos_f64(x);
    if abs_f64(c) < 1e-10 { return 0.0; }
    sin_f64(x) / c
}

pub fn atan2_f64(y: f64, x: f64) -> f64 {
    if abs_f64(x) < 1e-10 {
        if abs_f64(y) < 1e-10 { return 0.0; }
        return if y > 0.0 { core::f64::consts::FRAC_PI_2 } else { -core::f64::consts::FRAC_PI_2 };
    }
    let mut a = y / x;
    if abs_f64(a) > 1.0 {
        a = core::f64::consts::FRAC_PI_2 - atan2_f64(x, y);
    } else {
        let a2 = a * a;
        let a3 = a2 * a;
        let a5 = a3 * a2;
        let a7 = a5 * a2;
        a = a - a3 / 3.0 + a5 / 5.0 - a7 / 7.0;
    }
    if x < 0.0 {
        if y >= 0.0 { a + core::f64::consts::PI } else { a - core::f64::consts::PI }
    } else {
        a
    }
}

pub fn floor_f64(x: f64) -> f64 {
    let i = x as i64;
    if x < 0.0 && x != i as f64 { i as f64 - 1.0 } else { i as f64 }
}

pub fn ceil_f64(x: f64) -> f64 {
    let i = x as i64;
    if x > 0.0 && x != i as f64 { i as f64 + 1.0 } else { i as f64 }
}

pub fn round_f64(x: f64) -> f64 {
    floor_f64(x + 0.5)
}

pub fn deg_to_rad(deg: f64) -> f64 {
    deg * core::f64::consts::PI / 180.0
}

pub fn rad_to_deg(rad: f64) -> f64 {
    rad * 180.0 / core::f64::consts::PI
}
