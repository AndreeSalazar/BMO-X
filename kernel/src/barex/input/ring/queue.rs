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
    pub fn push(&mut self, _sqe: InputSqe) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}

impl<'a> InputCompletionQueue<'a> {
    pub fn pop(&mut self) -> Option<InputCqe> { None }
}
