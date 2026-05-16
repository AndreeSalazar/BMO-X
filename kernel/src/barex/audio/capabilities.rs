//! Capabilities de audio declaradas en `manifest.bef.toml`.

use crate::barex::abi::primitives::bx_u32;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct AudioCapabilities: bx_u32 {
        /// Permite abrir engine de salida (speakers/headphones).
        const PLAYBACK          = 1 << 0;
        /// Permite capturar (micrófono).
        const CAPTURE           = 1 << 1;
        /// Modo exclusive (toma del endpoint sin compartir).
        const EXCLUSIVE_MODE    = 1 << 2;
        /// Acceso al spatializer HRTF/Atmos.
        const SPATIAL           = 1 << 3;
        /// Procesamiento de efectos pesados (reverb largo, FFT).
        const HEAVY_DSP         = 1 << 4;
        /// Modo realtime (32 frames buffer → 0.7 ms).
        const REALTIME          = 1 << 5;
        /// MIDI in/out vía USB MIDI Class.
        const MIDI              = 1 << 6;
        /// Loopback (capturar el mix del sistema).
        const LOOPBACK          = 1 << 7;
    }
}
