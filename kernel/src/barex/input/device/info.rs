use crate::barex::abi::handle::BmoHandle;
use crate::barex::abi::primitives::{bx_u16, bx_u32};
use super::kind::DeviceKind;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DeviceInfo {
    pub handle: BmoHandle,
    pub kind: DeviceKind,
    pub _pad: [u8; 7],
    pub vendor_id: bx_u16,
    pub product_id: bx_u16,
    pub poll_rate_hz: bx_u32,
    /// Bus físico (`drivers/usb`, eventualmente `drivers/bluetooth`).
    pub bus_kind: bx_u32,
}

impl DeviceInfo {
    pub const REDRAGON_VID: bx_u16 = 0x0C45;
}
