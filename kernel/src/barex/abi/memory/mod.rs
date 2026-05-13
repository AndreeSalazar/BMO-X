//! `memory` — primitivas de memoria del BMO ABI.
//!
//! Reemplaza el patrón C `void* + size_t` y los punteros desnudos con
//! tipos seguros y compactos (caben en 2 GPRs del ABI).

pub mod slice;
pub mod range;
pub mod align;

pub use slice::{BmoSlice, BmoMutSlice};
pub use range::BmoRange;
pub use align::{align_up, align_down, BmoAligned};
