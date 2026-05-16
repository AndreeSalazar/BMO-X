use crate::barex::abi::primitives::{bx_u32, bx_u64};

#[repr(C, align(32))]
#[derive(Debug, Clone, Copy)]
pub struct InputCqe {
    pub user_data: bx_u64,
    /// Event ID o número de eventos en buffer.
    pub result: bx_u64,
    pub status: bx_u32,
    pub flags: bx_u32,
    pub _reserved: bx_u64,
}

impl InputCqe {
    pub const ZERO: Self = Self {
        user_data: 0, result: 0, status: 0, flags: 0, _reserved: 0,
    };
}
