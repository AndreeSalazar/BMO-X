//! `primitives` — tipos numéricos y booleanos del BMO ABI.
//!
//! Reemplaza `<stdint.h>`, `<stddef.h>` y `<stdbool.h>` con tipos garantizados,
//! sin sorpresas dependientes de la plataforma.

#![allow(non_camel_case_types)]

pub mod ints;
pub mod floats;
pub mod bool;

// Re-exports — `use crate::barex::abi::primitives::*;`
pub use ints::*;
pub use self::bool::*;
