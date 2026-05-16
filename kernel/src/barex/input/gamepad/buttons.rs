use crate::barex::abi::primitives::bx_u32;

bitflags::bitflags! {
    /// Mapeo canónico BMO (independiente del fabricante). Cada `xbox`/`ps`/
    /// `switch` traduce su layout físico a estos bits.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct GamepadButtons: bx_u32 {
        const SOUTH        = 1 << 0;  // A / Cross / B(NS)
        const EAST         = 1 << 1;  // B / Circle / A(NS)
        const WEST         = 1 << 2;  // X / Square / Y(NS)
        const NORTH        = 1 << 3;  // Y / Triangle / X(NS)
        const SHOULDER_L   = 1 << 4;  // LB / L1 / L
        const SHOULDER_R   = 1 << 5;  // RB / R1 / R
        const TRIGGER_L    = 1 << 6;  // analog trigger left pressed > thresh
        const TRIGGER_R    = 1 << 7;
        const SELECT       = 1 << 8;  // Back / Share / Minus
        const START        = 1 << 9;  // Menu / Options / Plus
        const STICK_L_CLICK= 1 << 10;
        const STICK_R_CLICK= 1 << 11;
        const DPAD_UP      = 1 << 12;
        const DPAD_DOWN    = 1 << 13;
        const DPAD_LEFT    = 1 << 14;
        const DPAD_RIGHT   = 1 << 15;
        const HOME         = 1 << 16; // Xbox button / PS button / Home
        const CAPTURE      = 1 << 17; // Share (NS) / Touchpad click (PS)
    }
}
