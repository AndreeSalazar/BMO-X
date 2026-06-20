//! `drivers::audio` — Audio driver layer.
//!
//! v1.3.0: Antes todo vivía en `barex::_blueprint::audio::*` esperando
//! Ring 3. Ahora:
//! - `dsp/` — algoritmos DSP funcionales (EQ, limiter, compressor, reverb)
//! - USB audio driver vive en `drivers::usb::audio` (ya existía)
//!
//! El día que llegue el audio engine completo, aquí se añadirá un
//! engine que conecte USB audio → DSP chain → output buffer.

pub mod dsp;
