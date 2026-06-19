//! Mapeo Nintendo Switch Pro Controller / Joy-Con → canónico BMO.
//!
//! Switch invierte South/East y West/North respecto a Xbox: B abajo, A
//! derecha. El traductor compensa para que el usuario siga viendo "south
//! es el botón confirmar" sin importar familia.

use crate::bmo_abi::primitives::bx_u16;

pub const VID_NINTENDO: bx_u16 = 0x057E;

pub const PID_SWITCH_PRO: bx_u16 = 0x2009;
pub const PID_JOYCON_L:   bx_u16 = 0x2006;
pub const PID_JOYCON_R:   bx_u16 = 0x2007;

#[inline(always)]
pub const fn is_switch(vid: u16, pid: u16) -> bool {
    vid == VID_NINTENDO
        && (pid == PID_SWITCH_PRO || pid == PID_JOYCON_L || pid == PID_JOYCON_R)
}
