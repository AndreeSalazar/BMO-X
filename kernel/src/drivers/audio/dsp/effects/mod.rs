//! DSP effects: EQ, limiter, compressor, reverb.
//!
//! Cada efecto implementa un patrón común:
//! - `new(...)` para crear instancia
//! - `process(&mut self, samples: &mut [f32])` para aplicar in-place
//! - `reset(&mut self)` para limpiar estado
//!
//! v1.6.15: dead_code allowed — HDA driver not yet wired (see DSP mod.rs).

#![allow(dead_code)]

pub mod compressor;
pub mod eq;
pub mod limiter;
pub mod reverb;
