//! Boxing/unboxing universal — para lenguajes managed.
//!
//! Convierte un primitivo BMO (u64 raw) en un "boxed value" de 16 bytes
//! que el lenguaje destino entiende (con header de tipo).
//!
//! Para Java: `(class_ptr: u64, value: u64)` — 16 bytes total.
//! Para Python: `(ob_type: u64, ob_value: u64)` — 16 bytes total.
//! Para C# / CLR: `(mt_ptr: u64, value: u64)` — 16 bytes total.
//! Para Rust: identidad (boxing solo vía `Box<T>` managed).
//!
//! Esta implementación cubre los lenguajes que ya adoptan layout BMO
//! (object = (type_id: u64, value: u64)). Lenguajes con layouts distintos
//! necesitan un Marshaller dedicado (registrado en `MarshalError`).

use crate::barex::{BxError, BxResult};
use crate::bmo_abi::primitives::bx_u64;
use crate::bmo_abi::type_system::TypeId;
use crate::bmo_abi::lang_bridge::ids;

/// Encajona un valor primitivo BMO en un objeto del lenguaje destino.
///
/// Output: 16 bytes en `dst_buf` con layout `[type_id: u64, value: u64]`.
///
/// `lang_id` debe ser uno de los IDs oficiales en `lang_bridge::ids`.
/// Lenguajes con layout distinto (Win32 VARIANT, Java Integer) aún
/// retornan `NotImplemented`.
pub fn box_value(value: bx_u64, src_type: TypeId, lang_id: u32,
                 dst_buf: &mut [u8; 16]) -> BxResult<bx_u64> {
    match lang_id {
        // Lenguajes que ya adoptan layout BMO (16B = type_id + value)
        ids::LANG_RUST
        | ids::LANG_C
        | ids::LANG_CPP
        | ids::LANG_ZIG
        | ids::LANG_SWIFT
        | ids::LANG_GO
        | ids::LANG_OCAML
        | ids::LANG_LUA
        | ids::LANG_NIM
        | ids::LANG_CRYSTAL
        | ids::LANG_DART
        | ids::LANG_KOTLIN
        | ids::LANG_ADA
        => {
            let type_id = src_type.raw();
            dst_buf[0..8].copy_from_slice(&type_id.to_le_bytes());
            dst_buf[8..16].copy_from_slice(&value.to_le_bytes());
            Ok(dst_buf.as_ptr() as u64)
        }
        // Lenguajes con layouts distintos: requiere marshaller específico.
        ids::LANG_JVM
        | ids::LANG_CLR
        | ids::LANG_PYTHON
        | ids::LANG_JS
        | ids::LANG_HASKELL
        | ids::LANG_BEAM
        | ids::LANG_RUBY
        | ids::LANG_PHP
        | ids::LANG_FORTRAN
        | ids::LANG_RACKET
        | ids::LANG_SCHEME
        | ids::LANG_CLOJURE
        => Err(BxError::NotImplemented),
        _ => Err(BxError::Unsupported),
    }
}

/// Desencajona un objeto del lenguaje origen a primitivo BMO.
///
/// Lee los 16 bytes en `obj_buf` con layout `[type_id: u64, value: u64]`.
pub fn unbox_value(obj_buf: &[u8; 16], src_lang: u32) -> BxResult<(bx_u64, TypeId)> {
    match src_lang {
        ids::LANG_RUST
        | ids::LANG_C
        | ids::LANG_CPP
        | ids::LANG_ZIG
        | ids::LANG_SWIFT
        | ids::LANG_GO
        | ids::LANG_OCAML
        | ids::LANG_LUA
        | ids::LANG_NIM
        | ids::LANG_CRYSTAL
        | ids::LANG_DART
        | ids::LANG_KOTLIN
        | ids::LANG_ADA
        => {
            let type_id = u64::from_le_bytes(obj_buf[0..8].try_into().unwrap_or([0u8; 8]));
            let value = u64::from_le_bytes(obj_buf[8..16].try_into().unwrap_or([0u8; 8]));
            Ok((value, TypeId::from_hash(type_id)))
        }
        _ => Err(BxError::Unsupported),
    }
}

/// Tamaño en bytes de un boxed value para el lenguaje dado.
pub fn boxed_size(lang_id: u32) -> BxResult<u32> {
    match lang_id {
        ids::LANG_RUST
        | ids::LANG_C
        | ids::LANG_CPP
        | ids::LANG_ZIG
        | ids::LANG_SWIFT
        | ids::LANG_GO
        | ids::LANG_OCAML
        | ids::LANG_LUA
        | ids::LANG_NIM
        | ids::LANG_CRYSTAL
        | ids::LANG_DART
        | ids::LANG_KOTLIN
        | ids::LANG_ADA
        => Ok(16),
        _ => Err(BxError::Unsupported),
    }
}
