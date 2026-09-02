//! `primitives` -- tipos numericos y booleanos del BMO ABI.
//!
//! Reemplaza `<stdint.h>`, `<stddef.h>` y `<stdbool.h>` con tipos garantizados,
//! sin sorpresas dependientes de la plataforma.
//!
//! -- EL SEMAFORO (L6g) y las dos preguntas de antes (L6e, L6f) --------
//!
//! Que cuesta que falle, por que falla ESTA pieza, y que arrastro si la
//! toco. La ley esta en `META-KERNEL_HARD.md`.
//!
//! [carril]  VERDE        el reparto de los alias. Hereda el verde de sus
//!                        tres
//! [cuesta]  NADA         son nombres de tipos
//! [riesgo]  ESPEJO       hereda de `ints.rs`: si un alias se separa del tipo
//!                        real, miente todo lo de encima

#![allow(non_camel_case_types)]

pub mod bool;
pub mod floats;
pub mod ints;

// Re-exports -- `use crate::bmo_abi::primitives::*;`
pub use self::bool::*;
pub use ints::*;
