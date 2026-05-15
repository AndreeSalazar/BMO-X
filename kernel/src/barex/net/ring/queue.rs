//! Anillos SQ/CQ. Lock-free SPSC entre app (productor SQ / consumidor CQ)
//! y kernel (consumidor SQ / productor CQ). Tamaño potencia de 2.

use crate::barex::{BxError, BxResult};
use crate::barex::abi::primitives::bx_u32;
use super::sqe::NetSqe;
use super::cqe::NetCqe;

pub struct NetSubmissionQueue<'a> {
    pub entries: &'a mut [NetSqe],
    pub head: bx_u32,
    pub tail: bx_u32,
}

pub struct NetCompletionQueue<'a> {
    pub entries: &'a mut [NetCqe],
    pub head: bx_u32,
    pub tail: bx_u32,
}

impl<'a> NetSubmissionQueue<'a> {
    pub fn push(&mut self, _sqe: NetSqe) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}

impl<'a> NetCompletionQueue<'a> {
    pub fn pop(&mut self) -> Option<NetCqe> {
        None
    }
}
