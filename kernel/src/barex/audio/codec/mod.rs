//! Decoders. Cada codec en su archivo (no monolitos).
//!
//! Reemplaza: Media Foundation Transforms (MFT), DirectShow filters,
//! GStreamer plugins, ffmpeg libavcodec (40 MB de bloat → BMO específico).

pub mod kind;
pub mod pcm;
pub mod opus;
pub mod vorbis;

