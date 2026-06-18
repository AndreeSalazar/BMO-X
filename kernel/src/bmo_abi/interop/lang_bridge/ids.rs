//! Identificadores estables de lenguajes soportados por BMO.
//!
//! Rango `0x0000_0000..=0x7FFF_FFFF` reservado para lenguajes oficiales.
//! Rango `0x8000_0000..=0xFFFF_FFFF` libre para experimentos / forks.

use crate::bmo_abi::primitives::bx_u32;

pub const LANG_UNKNOWN:    bx_u32 = 0x0000_0000;
pub const LANG_RUST:       bx_u32 = 0x0000_0001;
pub const LANG_C:          bx_u32 = 0x0000_0002;
pub const LANG_CPP:        bx_u32 = 0x0000_0003;
pub const LANG_ZIG:        bx_u32 = 0x0000_0004;
pub const LANG_SWIFT:      bx_u32 = 0x0000_0005;
pub const LANG_JVM:        bx_u32 = 0x0000_0006;
pub const LANG_CLR:        bx_u32 = 0x0000_0007;
pub const LANG_PYTHON:     bx_u32 = 0x0000_0008;
pub const LANG_JS:         bx_u32 = 0x0000_0009;
pub const LANG_GO:         bx_u32 = 0x0000_000A;
pub const LANG_OCAML:      bx_u32 = 0x0000_000B;
pub const LANG_LUA:        bx_u32 = 0x0000_000C;
pub const LANG_HASKELL:    bx_u32 = 0x0000_000D;
pub const LANG_BEAM:       bx_u32 = 0x0000_000E;
pub const LANG_NIM:        bx_u32 = 0x0000_000F;
pub const LANG_CRYSTAL:    bx_u32 = 0x0000_0010;
pub const LANG_DART:       bx_u32 = 0x0000_0011;
pub const LANG_KOTLIN:     bx_u32 = 0x0000_0012;
pub const LANG_RUBY:       bx_u32 = 0x0000_0013;
pub const LANG_PHP:        bx_u32 = 0x0000_0014;
pub const LANG_FORTRAN:    bx_u32 = 0x0000_0015;
pub const LANG_ADA:        bx_u32 = 0x0000_0016;
pub const LANG_RACKET:     bx_u32 = 0x0000_0017;
pub const LANG_SCHEME:     bx_u32 = 0x0000_0018;
pub const LANG_CLOJURE:    bx_u32 = 0x0000_0019;

/// Slot inicial para lenguajes futuros aún sin diseñar.
pub const LANG_FUTURE_START: bx_u32 = 0x0000_1000;

/// Frontera entre IDs oficiales y experimentales.
pub const LANG_EXPERIMENTAL_START: bx_u32 = 0x8000_0000;
