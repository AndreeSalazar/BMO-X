//! Capabilities de input declaradas en `manifest.bef.toml`.

use crate::barex::abi::primitives::bx_u32;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct InputCapabilities: bx_u32 {
        /// Lee teclado (poll de scancodes).
        const KEYBOARD          = 1 << 0;
        /// Lee mouse (deltas + botones).
        const MOUSE             = 1 << 1;
        /// Lee gamepads/HOTAS/wheel.
        const GAMEPAD           = 1 << 2;
        /// Lee botones del headset (volumen/mute).
        const HEADSET           = 1 << 3;
        /// Modo cursor capturado (FPS / raw deltas).
        const CURSOR_CAPTURE    = 1 << 4;
        /// HID raw — parsear reports arbitrarios (custom devices).
        const HID_RAW           = 1 << 5;
        /// Inyección de eventos sintéticos (testing, accesibilidad).
        const EVENT_INJECT      = 1 << 6;
        /// Hot-plug events (dispositivos enchufados/desenchufados).
        const HOT_PLUG          = 1 << 7;
        /// Rumble / force feedback (gamepads/wheels).
        const RUMBLE            = 1 << 8;
    }
}
