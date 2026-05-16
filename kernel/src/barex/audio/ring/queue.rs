use crate::barex::{BxError, BxResult};
use crate::barex::abi::primitives::bx_u32;
use super::sqe::AudioSqe;
use super::cqe::AudioCqe;

pub struct AudioSubmissionQueue<'a> {
    pub entries: &'a mut [AudioSqe],
    pub head: bx_u32,
    pub tail: bx_u32,
}

pub struct AudioCompletionQueue<'a> {
    pub entries: &'a mut [AudioCqe],
    pub head: bx_u32,
    pub tail: bx_u32,
}

impl<'a> AudioSubmissionQueue<'a> {
    pub fn push(&mut self, _sqe: AudioSqe) -> BxResult<()> {
        Err(BxError::NotImplemented)
    }
}

impl<'a> AudioCompletionQueue<'a> {
    pub fn pop(&mut self) -> Option<AudioCqe> { None }
}
