//! Item del Report Descriptor (1 byte tag + N bytes payload).

use crate::bmo_abi::primitives::{bx_u8, bx_u32};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemType {
    Main   = 0,
    Global = 1,
    Local  = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HidReportItem {
    pub item_type: bx_u8,
    pub tag: bx_u8,
    pub size: bx_u8,
    pub _pad: bx_u8,
    /// Payload little-endian zero-extended a 32-bit.
    pub data: bx_u32,
}

impl HidReportItem {
    pub const ZERO: Self = Self {
        item_type: 0, tag: 0, size: 0, _pad: 0, data: 0,
    };
}
