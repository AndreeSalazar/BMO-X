//! `AMD/` — Documentación técnica y lógica real del Ryzen 5 5600X.
//!
//! Ver `zen3/mod.rs` para el código real que RING 0 invoca.
//! Los archivos `.md` en este directorio son la documentación de referencia.

pub mod zen3;

// Re-exports del API público que RING 0 usa
pub use zen3::*;
