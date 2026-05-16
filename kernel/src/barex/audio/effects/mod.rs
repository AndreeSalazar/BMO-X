//! Efectos DSP. Cada uno en su archivo. Reemplaza APO chain de Windows
//! (Audio Processing Objects COM) y AU/LV2/VST3 plumbing.

pub mod kind;
pub mod eq;
pub mod reverb;
pub mod compressor;
pub mod limiter;

pub use kind::EffectKind;
pub use eq::BxEq;
pub use reverb::BxReverb;
pub use compressor::BxCompressor;
pub use limiter::BxLimiter;
