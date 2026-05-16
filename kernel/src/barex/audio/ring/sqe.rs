use crate::barex::abi::primitives::{bx_u8, bx_u32, bx_u64};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSqeOp {
    WriteBlock  = 1,
    ReadBlock   = 2,
    Pause       = 3,
    Resume      = 4,
    Drain       = 5,
    SetVolume   = 6,
}

#[repr(C, align(64))]
#[derive(Debug, Clone, Copy)]
pub struct AudioSqe {
    pub op: bx_u8,
    pub _pad: [bx_u8; 7],
    pub voice_or_engine: bx_u64,
    pub user_data: bx_u64,
    pub buf_ptr: bx_u64,
    pub buf_len: bx_u32,
    pub flags: bx_u32,
    pub _reserved: [bx_u64; 3],
}

impl AudioSqe {
    pub const ZERO: Self = Self {
        op: 0, _pad: [0; 7],
        voice_or_engine: 0, user_data: 0,
        buf_ptr: 0, buf_len: 0, flags: 0,
        _reserved: [0; 3],
    };
}
