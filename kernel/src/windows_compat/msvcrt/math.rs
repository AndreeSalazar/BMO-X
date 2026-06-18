//! msvcrt.dll — C math functions.
//!
//! Note: libm crate not available in no_std. These are stubs that
//! return approximate values. Real implementation would need a
//! software math library.

#![allow(dead_code)]

/// sin — sine function (stub).
#[no_mangle]
pub extern "C" fn sin(x: f64) -> f64 {
    // Taylor series approximation for small x
    x - (x * x * x) / 6.0 + (x * x * x * x * x) / 120.0
}

/// cos — cosine function (stub).
#[no_mangle]
pub extern "C" fn cos(x: f64) -> f64 {
    // Taylor series approximation for small x
    1.0 - (x * x) / 2.0 + (x * x * x * x) / 24.0
}

/// sqrt — square root (stub).
#[no_mangle]
pub extern "C" fn sqrt(x: f64) -> f64 {
    if x <= 0.0 { return 0.0; }
    // Newton's method approximation
    let mut guess = x / 2.0;
    for _ in 0..10 {
        guess = (guess + x / guess) / 2.0;
    }
    guess
}

/// floor — floor function.
#[no_mangle]
pub extern "C" fn floor(x: f64) -> f64 {
    let i = x as i64;
    if (i as f64) > x { (i - 1) as f64 } else { i as f64 }
}

/// ceil — ceiling function.
#[no_mangle]
pub extern "C" fn ceil(x: f64) -> f64 {
    let i = x as i64;
    if (i as f64) < x { (i + 1) as f64 } else { i as f64 }
}

/// pow — power function (stub).
#[no_mangle]
pub extern "C" fn pow(x: f64, y: f64) -> f64 {
    let _ = (x, y);
    // Stub: return 1.0 for now
    1.0
}

/// log — natural logarithm (stub).
#[no_mangle]
pub extern "C" fn log(x: f64) -> f64 {
    let _ = x;
    // Stub: return 0.0 for now
    0.0
}

/// log10 — base-10 logarithm (stub).
#[no_mangle]
pub extern "C" fn log10(x: f64) -> f64 {
    let _ = x;
    // Stub: return 0.0 for now
    0.0
}

/// abs — absolute value (integer).
#[no_mangle]
pub extern "C" fn abs(x: i32) -> i32 {
    if x < 0 { -x } else { x }
}

/// fabs — absolute value (float).
#[no_mangle]
pub extern "C" fn fabs(x: f64) -> f64 {
    if x < 0.0 { -x } else { x }
}
