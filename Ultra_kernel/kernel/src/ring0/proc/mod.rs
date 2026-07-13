//! Process / task / scheduler — minimal Ring 0 base.
//!
//! The full preemptive scheduler with SMP and full task structs
//! is a future-phase feature. For now we expose:
//!
//! - A single idle "task 0" (the boot CPU itself).
//! - A hook for the timer tick to call back into the kernel.
//! - `init()` that just logs that scheduling is in single-CPU mode.

pub mod task;
pub mod process;

pub use task::Task;
pub use process::Process;

/// Maximum number of tasks the (future) scheduler can hold.
/// Kept for ABI stability with the legacy kernel.
pub const MAX_TASKS: usize = 64;

/// Number of online CPUs. Hardcoded to 1 in the Ring 0 base.
pub fn online_cpu_count() -> u32 { 1 }

/// Initialize the scheduler. Currently a no-op.
pub fn init() {
    crate::ring0::dev::console::serial_write("[proc] scheduler init (single-CPU idle task)\n");
    crate::ring0::dev::console::serial_write("[proc] SMP online CPUs: 1 (multi-core deferred)\n");
}

/// Tick callback from the timer IRQ. Stub.
pub fn timer_tick() {
    // No-op: no tasks to schedule yet.
}
