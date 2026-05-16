//! Mapeo Xbox One/Series → `GamepadButtons` canónico BMO.

use crate::barex::abi::primitives::bx_u16;

/// VID Microsoft Xbox controller.
pub const VID_MICROSOFT: bx_u16 = 0x045E;

/// PIDs de los controllers Xbox modernos (One, Series X/S).
pub const PID_XBOX_ONE_CONTROLLER:    bx_u16 = 0x02D1;
pub const PID_XBOX_SERIES_CONTROLLER: bx_u16 = 0x0B12;

/// True si VID/PID corresponde a un Xbox oficial.
#[inline(always)]
pub const fn is_xbox(vid: u16, pid: u16) -> bool {
    vid == VID_MICROSOFT
        && (pid == PID_XBOX_ONE_CONTROLLER || pid == PID_XBOX_SERIES_CONTROLLER)
}
