//! Mapeo DualSense / DualShock 4 → `GamepadButtons` canónico BMO.

use crate::barex::abi::primitives::bx_u16;

pub const VID_SONY: bx_u16 = 0x054C;

pub const PID_DUALSHOCK4_V2: bx_u16 = 0x09CC;
pub const PID_DUALSENSE:     bx_u16 = 0x0CE6;
pub const PID_DUALSENSE_EDGE:bx_u16 = 0x0DF2;

#[inline(always)]
pub const fn is_playstation(vid: u16, pid: u16) -> bool {
    vid == VID_SONY
        && (pid == PID_DUALSHOCK4_V2 || pid == PID_DUALSENSE || pid == PID_DUALSENSE_EDGE)
}
