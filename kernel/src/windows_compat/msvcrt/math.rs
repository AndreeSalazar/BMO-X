//! msvcrt.dll — C math functions.

#![allow(dead_code)]

/// sin — sine function.
#[no_mangle]
pub extern "C" fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// cos — cosine function.
#[no_mangle]
pub extern "C" fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// sqrt — square root.
#[no_mangle]
pub extern "C" fn sqrt(x: f64) -> f64 {
    libm::sqrt(x)
}

/// floor — floor function.
#[no_mangle]
pub extern "C" fn floor(x: f64) -> f64 {
    libm::floor(x)
}

/// ceil — ceiling function.
#[no_mangle]
pub extern "C" fn ceil(x: f64) -> f64 {
    libm::ceil(x)
}

/// pow — power function.
#[no_mangle]
pub extern "C" fn pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

/// log — natural logarithm.
#[no_mangle]
pub extern "C" fn log(x: f64) -> f64 {
    libm::log(x)
}

/// log10 — base-10 logarithm.
#[no_mangle]
pub extern "C" fn log10(x: f64) -> f64 {
    libm::log10(x)
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
