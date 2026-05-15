//! `marshal` — conversión bidireccional Lang ↔ BMO ↔ Lang.
//!
//! Cuando un módulo BEF compilado en lenguaje A llama a uno compilado en
//! lenguaje B, los argumentos pasan por:
//!
//! ```text
//!  A → marshal::to_bmo()   → BMO ABI canónico → marshal::from_bmo() → B
//! ```
//!
//! La mayoría de tipos BMO son ya FFI-estables, así que el marshal es a
//! menudo identidad. Sólo se necesita transformación cuando el lenguaje
//! tiene boxing (Java Integer), tagged values (JS), o convenciones de
//! string distintas (UTF-16).
//!
//! ## Casos cubiertos
//!
//! - Boxing/unboxing (Java Integer ↔ `bx_i32`).
//! - String enc: UTF-8 ↔ UTF-16 (Win32) ↔ ASCII C-string.
//! - Bool: 1 byte BMO ↔ 4 bytes Win32 BOOL.
//! - Closures: `BmoClosure` ↔ method-pointer + `this` (C++/.NET).
//! - Errors: `BmoStatus` ↔ `HRESULT` ↔ `errno` ↔ Java exception.

#![allow(dead_code)]

pub mod cast;
pub mod boxing;
pub mod string_enc;
pub mod boolean;

pub use cast::{Marshaller, MarshalError};
pub use boxing::{box_value, unbox_value};
pub use string_enc::{utf8_to_utf16_estimate, utf16_to_utf8_estimate};
pub use boolean::{bool_to_bmo, bmo_to_bool, win32_bool_to_bmo};
