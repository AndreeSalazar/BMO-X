//! Submission Queue Entry — 64 B, cache-line alineado.

use crate::bmo_abi::primitives::{bx_u8, bx_u32, bx_u64};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetSqeOp {
    Connect    = 1,
    Send       = 2,
    Recv       = 3,
    SendTo     = 4,
    RecvFrom   = 5,
    Accept     = 6,
    Close      = 7,
    QuicWrite  = 8,
    QuicRead   = 9,
    DnsResolve = 10,
}

#[repr(C, align(64))]
#[derive(Debug, Clone, Copy)]
pub struct NetSqe {
    pub op: bx_u8,
    pub _pad: [bx_u8; 7],
    /// Handle del socket / endpoint / stream.
    pub handle: bx_u64,
    /// User data (opaca, vuelve en CQE.user_data).
    pub user_data: bx_u64,
    /// Puntero a buffer (envío o recepción).
    pub buf_ptr: bx_u64,
    /// Longitud del buffer.
    pub buf_len: bx_u32,
    /// Flags (per-op).
    pub flags: bx_u32,
    /// Reservado para extensión sin romper layout.
    pub _reserved: [bx_u64; 3],
}

impl NetSqe {
    pub const ZERO: Self = Self {
        op: 0, _pad: [0; 7],
        handle: 0, user_data: 0,
        buf_ptr: 0, buf_len: 0, flags: 0,
        _reserved: [0; 3],
    };
}
