//! DSP effects: EQ, limiter, compressor, reverb.
//!
//! Cada efecto implementa un patrón común:
//! - `new(...)` para crear instancia
//! - `process(&mut self, samples: &mut [f32])` para aplicar in-place
//! - `reset(&mut self)` para limpiar estado

pub mod compressor;
pub mod eq;
pub mod limiter;
pub mod reverb;
