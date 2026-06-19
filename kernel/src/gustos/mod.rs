//! `gustos` — Sistema de audio procedural de FastOS.
//!
//! v1.5.0: reproduce sonidos icónicos de Windows + música procedural
//! usando FM synthesis puro. No usa samples.
//!
//! ## Uso
//!
//! ```rust
//! use crate::gustos::tracks::windows;
//!
//! windows::startup();  // Suena el boot de Windows-inspired
//! ```

pub mod synth;
pub mod tracks;

pub use synth::fm::{Adsr, FmParams};
pub use synth::pcm::{silence, OutputMode, set_output_mode};
