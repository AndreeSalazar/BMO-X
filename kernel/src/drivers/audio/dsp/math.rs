//! Funciones matemáticas para DSP en `no_std`.
//! Aproximaciones polynomial precisas para audio.

const FRAC_PI_2: f32 = core::f32::consts::FRAC_PI_2;
const PI: f32 = core::f32::consts::PI;
const LN_2: f32 = 0.6931471805599453;
const FRAC_1_LN_2: f32 = 1.4426950408889634;

pub fn dsp_sin(mut x: f32) -> f32 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    x = x.abs();
    let mut n = (x / PI) as i32;
    x -= n as f32 * PI;
    if x > FRAC_PI_2 {
        x = PI - x;
    }
    if x > FRAC_PI_2 {
        x -= PI;
        n += 1;
    }
    let x2 = x * x;
    let mut result = x;
    result += x * x2 * -0.16666666666666666;
    result += x * x2 * x2 * 0.008333333333333333;
    result += x * x2 * x2 * x2 * -0.0001984126984126984;
    result += x * x2 * x2 * x2 * x2 * 0.000002755731922398589;
    result += x * x2 * x2 * x2 * x2 * x2 * -0.00000002505210838544172;
    result += x * x2 * x2 * x2 * x2 * x2 * x2 * 0.00000000016059043836821614;
    if (n & 1) == 1 {
        result = -result;
    }
    sign * result
}

pub fn dsp_cos(x: f32) -> f32 {
    dsp_sin(x + FRAC_PI_2)
}

pub fn dsp_exp(mut x: f32) -> f32 {
    if x > 88.0 {
        return 1.0e38;
    }
    if x < -88.0 {
        return 0.0;
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    x = x.abs();
    let k = (x * FRAC_1_LN_2) as i32;
    let r = x - k as f32 * LN_2;
    let mut term = r;
    let mut result = 1.0 + term;
    term *= r / 2.0;
    result += term;
    term *= r / 3.0;
    result += term;
    term *= r / 4.0;
    result += term;
    term *= r / 5.0;
    result += term;
    term *= r / 6.0;
    result += term;
    term *= r / 7.0;
    result += term;
    term *= r / 8.0;
    result += term;
    result *= dsp_pow2i(k);
    if sign < 0.0 {
        1.0 / result
    } else {
        result
    }
}

fn dsp_pow2i(k: i32) -> f32 {
    let bits = ((k + 127) as u32) << 23;
    f32::from_bits(bits)
}

pub fn dsp_ln(x: f32) -> f32 {
    if x <= 0.0 {
        return -1.0e38;
    }
    let bits = x.to_bits();
    let exp = ((bits >> 23) as i32) - 127;
    let mantissa_bits = bits & 0x007F_FFFF;
    let mantissa = f32::from_bits(mantissa_bits | 0x3F80_0000);
    let m = mantissa - 1.0;
    let mut term = m;
    let mut result = term;
    term *= -m / 2.0;
    result += term;
    term *= m / 3.0;
    result += term;
    term *= -m / 4.0;
    result += term;
    term *= m / 5.0;
    result += term;
    term *= -m / 6.0;
    result += term;
    result += exp as f32 * LN_2;
    result
}

pub fn dsp_log2(x: f32) -> f32 {
    dsp_ln(x) * FRAC_1_LN_2
}

pub fn dsp_powf(base: f32, exponent: f32) -> f32 {
    if base <= 0.0 {
        return 0.0;
    }
    dsp_exp(exponent * dsp_ln(base))
}

pub fn dsp_abs(x: f32) -> f32 {
    if x < 0.0 { -x } else { x }
}

pub fn dsp_max(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

pub fn dsp_min(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

pub fn dsp_sqrt(x: f32) -> f32 {
    if x <= 0.0 { return 0.0; }
    // Newton-Raphson: 3 iterations gives ~24 bits of precision
    let mut guess = f32::from_bits((x.to_bits() >> 1) + 0x1FC00000); // rough estimate
    for _ in 0..4 {
        guess = (guess + x / guess) * 0.5;
    }
    guess
}

pub fn dsp_acos(x: f32) -> f32 {
    let clamped = if x < -1.0 { -1.0 } else if x > 1.0 { 1.0 } else { x };
    // Polynomial approximation: acos(x) ≈ π/2 - x - x³/6 - 3x⁵/40 - 5x⁷/112
    let x2 = clamped * clamped;
    let x3 = x2 * clamped;
    let x5 = x3 * x2;
    let x7 = x5 * x2;
    let x9 = x7 * x2;
    core::f32::consts::FRAC_PI_2
        - clamped
        - x3 * 0.16666666666666666
        - x5 * 0.075
        - x7 * 0.04464285714285714
        - x9 * 0.030864197530864196
}

pub fn dsp_atan2(y: f32, x: f32) -> f32 {
    if x > 0.0 {
        dsp_atan(y / x)
    } else if x < 0.0 && y >= 0.0 {
        dsp_atan(y / x) + core::f32::consts::PI
    } else if x < 0.0 && y < 0.0 {
        dsp_atan(y / x) - core::f32::consts::PI
    } else if x == 0.0 && y > 0.0 {
        core::f32::consts::FRAC_PI_2
    } else if x == 0.0 && y < 0.0 {
        -core::f32::consts::FRAC_PI_2
    } else {
        0.0
    }
}

fn dsp_atan(x: f32) -> f32 {
    let ax = dsp_abs(x);
    if ax <= 1.0 {
        // atan(x) ≈ x - x³/3 + x⁵/5 - x⁷/7 + x⁹/9
        let x2 = x * x;
        let x3 = x2 * x;
        let x5 = x3 * x2;
        let x7 = x5 * x2;
        let x9 = x7 * x2;
        x - x3 * 0.3333333333333333 + x5 * 0.2 - x7 * 0.14285714285714285 + x9 * 0.1111111111111111
    } else {
        // atan(x) = π/2 - atan(1/x) for |x| > 1
        let sign = if x > 0.0 { 1.0 } else { -1.0 };
        sign * (core::f32::consts::FRAC_PI_2 - dsp_atan(1.0 / ax))
    }
}
