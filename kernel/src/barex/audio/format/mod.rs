//! Formatos PCM y métricas relacionadas. Reemplaza el zoo `WAVEFORMATEX`
//! / `WAVEFORMATEXTENSIBLE` / `AudioStreamBasicDescription` (CoreAudio).

pub mod sample;
pub mod channels;
pub mod latency;

pub use sample::SampleFormat;
pub use channels::ChannelLayout;
