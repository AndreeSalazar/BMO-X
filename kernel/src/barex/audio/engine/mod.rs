//! Engine raíz de `bx_audio`. Reemplaza `IAudioClient3` (Win32 COM).

pub mod engine;
pub mod mode;
pub mod backend_kind;

pub use engine::BxAudioEngine;
pub use mode::EngineMode;
pub use backend_kind::AudioBackend;
