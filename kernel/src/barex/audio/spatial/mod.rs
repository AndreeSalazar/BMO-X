//! Espacialización (HRTF / Atmos / Ambisonics). Reemplaza Microsoft
//! Spatial Audio API y XAudio2 X3DAudio.

pub mod spatializer;
pub mod listener;

pub use spatializer::BxSpatializer;
