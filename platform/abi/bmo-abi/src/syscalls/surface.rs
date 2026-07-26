//! BMO ABI core syscall surface (the frozen 3-call surface).
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
/// `INVOKE(CURRENT_TASK, CHANNEL_OPEN, index)` → the caller's estuary
/// capability handle for BMO Channel `index`. Fails with NEEDS_CAP when
/// the process was not granted that estuary.
pub const TASK_OP_CHANNEL_OPEN: u64 = 0x05;
/// `INVOKE(CURRENT_TASK, CONSOLE_WRITE, packed)` → emit up to 8 bytes of
/// text (packed little-endian in `packed`, NUL-terminated within the word)
/// to the kernel bootstrap console. This is the debug door that lets the
/// very first Ring 3 program prove the CPL3→CPL0 path visually before a
/// real console capability/estuary service exists; it will migrate to a
/// console handle once the display server lands.
pub const TASK_OP_CONSOLE_WRITE: u64 = 0x06;

/// Crea un endpoint atendido por este proceso: `arg0` es el estuario por el
/// que se le entregaran las llamadas, y devuelve el handle del endpoint.
///
/// Es lo unico que Endpoint RPC anade a la superficie. Llamar, atender y
/// responder NO son operaciones nuevas: son lo que `INVOKE` y `WAIT` ya
/// significan cuando el handle resuelve a un endpoint o a un reply. La
/// superficie sigue siendo de tres puertas.
pub const TASK_OP_ENDPOINT_CREATE: u64 = 0x07;

/// Operations accepted by `CURRENT_TASK`.
pub mod task_op {
    pub const GET_PID: u64 = super::TASK_OP_GET_PID;
    pub const ENDPOINT_CREATE: u64 = super::TASK_OP_ENDPOINT_CREATE;
    pub const GET_TID: u64 = super::TASK_OP_GET_TID;
    pub const YIELD: u64 = super::TASK_OP_YIELD;
    pub const EXIT: u64 = super::TASK_OP_EXIT;
    pub const CHANNEL_OPEN: u64 = super::TASK_OP_CHANNEL_OPEN;
    pub const CONSOLE_WRITE: u64 = super::TASK_OP_CONSOLE_WRITE;
}

/// `INVOKE` operations accepted by a channel (estuary) capability.
pub const CHANNEL_OP_GET_SEQ: u64 = 0x01;
pub const CHANNEL_OP_GET_INDEX: u64 = 0x02;

pub mod channel_op {
    /// Completion-side sequence — the value `WAIT` compares against.
    pub const GET_SEQ: u64 = super::CHANNEL_OP_GET_SEQ;
    /// Estuary index backing this capability.
    pub const GET_INDEX: u64 = super::CHANNEL_OP_GET_INDEX;
}

/// Translate the temporary v1 task surface into its v2 capability operation.
///
/// This belongs at the ABI boundary so compilers and runtimes do not each
/// duplicate a legacy-number mapping. It can be removed with the v1 table.
pub const fn task_operation_for_legacy_syscall(number: u32) -> Option<u64> {
    match number {
        super::NR_PROC_GET_PID => Some(TASK_OP_GET_PID),
        super::NR_PROC_GET_TID | super::NR_THREAD_SELF => Some(TASK_OP_GET_TID),
        super::NR_PROC_YIELD => Some(TASK_OP_YIELD),
        super::NR_PROC_EXIT | super::NR_THREAD_EXIT => Some(TASK_OP_EXIT),
        _ => None,
    }
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

/// `WAIT(waitable, observed_sequence, timeout_ns)`.
///
/// Blocks until the waitable's sequence moves past `observed_sequence`
/// or `timeout_ns` elapses (0 = no timeout). `waitable = 0` is a pure
/// timed sleep. The kernel compares the sequence under its scheduler
/// lock, so a kick can never be lost between the caller's read and the
/// block. On resume, re-read the shared sequence — the returned value
/// is advisory.
#[inline(always)]
pub unsafe fn wait(
    waitable: u64,
    observed_sequence: u64,
    timeout_ns: u64,
) -> SyscallResult {
    syscall3(NR_WAIT, waitable, observed_sequence, timeout_ns)
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

    #[test]
    fn legacy_task_translation_has_one_canonical_mapping() {
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_PROC_EXIT), Some(TASK_OP_EXIT));
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_PROC_GET_PID), Some(TASK_OP_GET_PID));
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_PROC_GET_TID), Some(TASK_OP_GET_TID));
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_PROC_YIELD), Some(TASK_OP_YIELD));
        assert_eq!(task_operation_for_legacy_syscall(super::super::NR_FS_OPEN), None);
    }
}
