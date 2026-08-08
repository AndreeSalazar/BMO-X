//! `primitives` -- tipos numericos y booleanos del BMO ABI.
//!
//! Reemplaza `<stdint.h>`, `<stddef.h>` y `<stdbool.h>` con tipos garantizados,
//! sin sorpresas dependientes de la plataforma.

#![allow(non_camel_case_types)]

pub mod bool;
pub mod floats;
pub mod ints;

// Re-exports -- `use crate::bmo_abi::primitives::*;`
pub use self::bool::*;
pub use ints::*;
