//! System service -- estuary 0. The first real capability service.
//!
//! By-value operations only (no user pointers): liveness, time and
//! scheduler telemetry. The completion entry echoes the request opcode
//! so Ring 3 can correlate responses.

/// Liveness probe: completion echoes `(PING, a0, a1, a2)` untouched.
pub const OP_PING: u64 = 0x01;
/// Timer ticks since boot: completion `(TICKS, ticks, 0, 0)`.
pub const OP_TICKS: u64 = 0x02;
/// Timestamp counter: completion `(TSC, rdtsc, tsc_hz, 0)`.
pub const OP_TSC: u64 = 0x03;
/// Scheduler telemetry: completion `(TASKS, total, runnable, 0)`.
pub const OP_TASKS: u64 = 0x04;

/// Unknown opcodes complete as `(opcode, ERROR_UNSUPPORTED, 0, 0)` so a
/// client never blocks on a request that will not be answered.
const ERROR_UNSUPPORTED: u64 = 10;

pub fn handle(opcode: u64, a0: u64, a1: u64, a2: u64) -> Option<(u64, u64, u64, u64)> {
    match opcode {
        OP_PING => Some((OP_PING, a0, a1, a2)),
        OP_TICKS => Some((OP_TICKS, crate::ring0::plat::timer::ticks(), 0, 0)),
        OP_TSC => Some((
            OP_TSC,
            crate::ring0::task::scheduler::rdtsc(),
            crate::ring0::task::scheduler::tsc_freq(),
            0,
        )),
        OP_TASKS => {
            let (total, runnable) = crate::ring0::task::scheduler::counts();
            Some((OP_TASKS, total as u64, runnable as u64, 0))
        }
        _ => Some((opcode, ERROR_UNSUPPORTED, 0, 0)),
    }
}
