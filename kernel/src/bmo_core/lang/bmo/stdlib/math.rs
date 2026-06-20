//! ÑEXO std::math — Aritmética.

#![allow(dead_code)]

pub fn abs(a: i64) -> i64 { if a < 0 { -a } else { a } }
pub fn min(a: i64, b: i64) -> i64 { if a < b { a } else { b } }
pub fn max(a: i64, b: i64) -> i64 { if a > b { a } else { b } }
pub fn clamp(v: i64, lo: i64, hi: i64) -> i64 { min(max(v, lo), hi) }
pub fn pow(base: u64, exp: u32) -> u64 { let mut r = 1u64; let mut i = 0; while i < exp { r = r.wrapping_mul(base); i += 1; } r }
pub fn sqrt(n: u64) -> u64 { let mut x = n; let mut y = (x + 1) / 2; while y < x { x = y; y = (x + n / x) / 2; } x }
