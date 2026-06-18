//! Anillos SQ/CQ. Lock-free SPSC entre app (productor SQ / consumidor CQ)
//! y kernel (consumidor SQ / productor CQ). Tamaño potencia de 2.

use crate::barex::{BxError, BxResult};
use crate::bmo_abi::primitives::bx_u32;
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
    pub fn push(&mut self, sqe: NetSqe) -> BxResult<()> {
        let cap = self.entries.len() as bx_u32;
        let next = (self.head + 1) & (cap - 1);
        if next == self.tail {
            return Err(BxError::BufferTooSmall);
        }
        self.entries[self.head as usize] = sqe;
        self.head = next;
        Ok(())
    }
}

impl<'a> NetCompletionQueue<'a> {
    pub fn pop(&mut self) -> Option<NetCqe> {
        if self.tail == self.head {
            return None;
        }
        let cap = self.entries.len() as bx_u32;
        let val = self.entries[self.tail as usize];
        self.tail = (self.tail + 1) & (cap - 1);
        Some(val)
    }
}
