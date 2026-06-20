//! `drivers::audio::dsp` — math helpers (v1.7.4).
//!
//! v1.7.4: sólo `math.rs` con `dsp_sin` (usado por `gustos::synth::fm`).
//! Los effects (eq, limiter, compressor, reverb) se eliminaron — no
//! hay HDA wired y nadie los llama.

pub mod math;
