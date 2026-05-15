//! Completion Queue Entry — 32 B.

use crate::barex::abi::primitives::{bx_u32, bx_u64};

#[repr(C, align(32))]
#[derive(Debug, Clone, Copy)]
pub struct NetCqe {
    /// Echo del `user_data` del SQE.
    pub user_data: bx_u64,
    /// Resultado: bytes transferidos, o error code negativo.
    pub result: bx_u64,
    /// Status BMO (`BmoStatus.code`).
    pub status: bx_u32,
    pub flags: bx_u32,
    pub _reserved: bx_u64,
}

impl NetCqe {
    pub const ZERO: Self = Self {
        user_data: 0, result: 0, status: 0, flags: 0, _reserved: 0,
    };
}
