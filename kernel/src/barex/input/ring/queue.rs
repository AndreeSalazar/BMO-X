use crate::barex::{BxError, BxResult};
use crate::barex::abi::primitives::bx_u32;
use super::sqe::InputSqe;
use super::cqe::InputCqe;

pub struct InputSubmissionQueue<'a> {
    pub entries: &'a mut [InputSqe],
    pub head: bx_u32,
    pub tail: bx_u32,
}

pub struct InputCompletionQueue<'a> {
    pub entries: &'a mut [InputCqe],
    pub head: bx_u32,
    pub tail: bx_u32,
}

impl<'a> InputSubmissionQueue<'a> {
    pub fn push(&mut self, sqe: InputSqe) -> BxResult<()> {
        let len = self.entries.len() as bx_u32;
        if len == 0 {
            return Err(BxError::BufferTooSmall);
        }
        let next = (self.head + 1) % len;
        if next == self.tail {
            return Err(BxError::BufferTooSmall);
        }
        self.entries[self.head as usize] = sqe;
        self.head = next;
        Ok(())
    }
}

impl<'a> InputCompletionQueue<'a> {
    pub fn pop(&mut self) -> Option<InputCqe> {
        let len = self.entries.len() as bx_u32;
        if len == 0 || self.head == self.tail {
            return None;
        }
        let cqe = self.entries[self.tail as usize];
        self.tail = (self.tail + 1) % len;
        Some(cqe)
    }
}
