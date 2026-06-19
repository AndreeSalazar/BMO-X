//! `drivers::audio::dsp` — Digital Signal Processing primitives.
//!
//! v1.3.0: Movido desde `barex::_blueprint::audio::effects::*`.
//! Antes vivía en el blueprint esperando Ring 3 — ahora vive en el
//! driver layer porque estos algoritmos son **funcionales** y se
//! pueden usar desde el kernel directamente.
//!
//! ## Estructura
//!
//! ```text
//!   dsp/
//!   ├── math.rs              ← sin/cos/sqrt/exp/log/pow en no_std
//!   ├── effects/
//!   │   ├── eq.rs            ← EQ paramétrico 10 bandas
//!   │   ├── limiter.rs       ← Brick-wall limiter con lookahead
//!   │   ├── compressor.rs    ← Compresor de dinámica
//!   │   └── reverb.rs        ← Reverb 8-delay (Schroeder-like)
//!   └── mod.rs               ← este archivo
//! ```
//!
//! ## Uso
//!
//! Estos módulos están diseñados para procesar buffers `&[f32]`
//! in-place. No alocan, son `no_std + alloc` friendly.

pub mod math;
pub mod effects;
