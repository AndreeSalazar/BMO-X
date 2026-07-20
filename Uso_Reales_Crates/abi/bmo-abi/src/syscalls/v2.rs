//! BMO ABI v2 core syscall contract.
//!
//! Services such as files, network, audio, graphics and input are capability
//! operations transported through BMO Channel. They are not kernel syscalls.

use super::{syscall2, syscall3, syscall6, SyscallResult};

/// Synchronous, capability-scoped control operation.
pub const NR_INVOKE: u32 = 0x00;
/// Notify a channel consumer after publishing submissions.
pub const NR_CHANNEL_KICK: u32 = 0x01;
/// Block until a sequence changes or an absolute deadline expires.
pub const NR_WAIT: u32 = 0x02;
pub const CORE_SYSCALL_COUNT: usize = 3;

/// Process-local pseudo-handle that always resolves to the calling task.
/// It grants no authority over another task and must never be transferred.
pub const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;
pub const TASK_OP_GET_PID: u64 = 0x01;
pub const TASK_OP_GET_TID: u64 = 0x02;
pub const TASK_OP_YIELD: u64 = 0x03;
pub const TASK_OP_EXIT: u64 = 0x04;

/// Operations accepted by `CURRENT_TASK`.
pub mod task_op {
    pub const GET_PID: u64 = super::TASK_OP_GET_PID;
    pub const GET_TID: u64 = super::TASK_OP_GET_TID;
    pub const YIELD: u64 = super::TASK_OP_YIELD;
    pub const EXIT: u64 = super::TASK_OP_EXIT;
}

/// `INVOKE(capability, operation, a0, a1, a2, a3)`.
#[inline(always)]
pub unsafe fn invoke(
    capability: u64,
    operation: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
) -> SyscallResult {
    syscall6(NR_INVOKE, capability, operation, a0, a1, a2, a3)
}

/// `CHANNEL_KICK(channel, published_sequence)`.
#[inline(always)]
pub unsafe fn channel_kick(channel: u64, published_sequence: u64) -> SyscallResult {
    syscall2(NR_CHANNEL_KICK, channel, published_sequence)
}

/// `WAIT(waitable, observed_sequence, absolute_deadline_ns)`.
#[inline(always)]
pub unsafe fn wait(
    waitable: u64,
    observed_sequence: u64,
    absolute_deadline_ns: u64,
) -> SyscallResult {
    syscall3(NR_WAIT, waitable, observed_sequence, absolute_deadline_ns)
}

pub const fn name(number: u32) -> Option<&'static str> {
    match number {
        NR_INVOKE => Some("bmo_invoke"),
        NR_CHANNEL_KICK => Some("bmo_channel_kick"),
        NR_WAIT => Some("bmo_wait"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_surface_is_frozen_to_three_calls() {
        assert_eq!(CORE_SYSCALL_COUNT, 3);
        assert_eq!(name(0), Some("bmo_invoke"));
        assert_eq!(name(1), Some("bmo_channel_kick"));
        assert_eq!(name(2), Some("bmo_wait"));
        assert_eq!(name(3), None);
    }
}
