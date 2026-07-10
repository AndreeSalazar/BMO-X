//! Scheduler de BMO — Round-Robin con prioridades.
//!
//! Diseñado para Ryzen 5 5600X (1 CCD × 6 cores × 2 threads).
//! Integra con APIC timer para preemptive scheduling.
//!
//! Modular:
//!   - process:  Process management
//!   - task:     Task management + ctx switching
//!   - user_init: Ring 3 process loading
//!
//! v1.8.7: eliminado `rt` (Real-time EDF scheduler) — sin consumidores
//! activos. Restaurar desde git cuando se reactive el caso RT (audio
//! HDA, render de juego).


pub mod process;
pub mod task;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Audio/input: garantía de latencia sub-ms.
    Realtime,
    /// Render thread del juego.
    HighGame,
    /// Threads de juego normales.
    Game,
    /// Apps interactivas (UI).
    Interactive,
    /// Background.
    Idle,
}

impl Priority {
    /// Quantum length (in APIC timer ticks at the configured frequency).
    pub fn quantum(self) -> u32 {
        match self {
            Priority::Realtime    => 1,
            Priority::HighGame    => 5,
            Priority::Game        => 10,
            Priority::Interactive => 20,
            Priority::Idle        => 50,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CoreAffinity {
    /// Bitmask de los 12 hilos del 5600X (0..=11).
    pub mask: u16,
}

impl CoreAffinity {
    pub const ANY: Self = Self { mask: 0x0FFF };
    /// Cores físicos solamente (sin SMT).
    pub const PHYSICAL_ONLY: Self = Self { mask: 0b0000_0101_0101_0101 };
}

/// Total number of scheduler ticks since boot. Useful for diagnostic
/// or for `sleep` in userland (number of ticks to wait).
static mut TOTAL_TICKS: u64 = 0;

/// Number of times the scheduler found NO ready task. Each idle tick
/// burns one APIC interrupt worth of CPU. Exposed for tests and
/// profiling.
static mut IDLE_TICKS: u64 = 0;

/// Called from APIC timer interrupt — performs preemptive scheduling.
pub fn timer_tick() {
    unsafe { TOTAL_TICKS = TOTAL_TICKS.saturating_add(1); }

    if let Some(current) = task::current() {
        if current.time_slice > 0 {
            current.time_slice -= 1;
        }
        if current.time_slice == 0 {
            current.state = task::State::Ready;
            schedule();
        }
    } else {
        // No task running — try to find one; if none, count idle.
        if task::pick_next().is_none() {
            unsafe { IDLE_TICKS = IDLE_TICKS.saturating_add(1); }
        } else {
            schedule();
        }
    }
}

/// Pick next task and switch to it.
pub fn schedule() {
    let current_idx = task::current_index();

    let Some(next_idx) = task::pick_next() else {
        return;
    };

    if next_idx == current_idx {
        if let Some(t) = task::get(next_idx) {
            t.state = task::State::Running;
            t.time_slice = t.priority.quantum();
        }
        return;
    }

    if let Some(cur) = task::get(current_idx) {
        if cur.state == task::State::Running {
            cur.state = task::State::Ready;
        }
    }

    if let Some(next) = task::get(next_idx) {
        next.state = task::State::Running;
        next.time_slice = next.priority.quantum();

        // ── Privilege transition: must be done BEFORE iretq ──
        if next.kernel_stack_top == 0 {
            // Safety net: should never happen (task::alloc now assigns a stack).
            crate::dev::console::serial_write("[sched] FATAL: next task has no kernel stack\n");
            unsafe { core::arch::asm!("cli"); }
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
        crate::arch::gdt::set_kernel_stack(next.kernel_stack_top);
        crate::arch::syscall::set_syscall_kernel_stack(next.kernel_stack_top);

        // ── Address space switch ──
        if let Some(proc) = process::get_process(next.pid) {
            if proc.page_table_root != 0 {
                let current_cr3 = crate::mm::virt::read_cr3();
                if proc.page_table_root != current_cr3 {
                    // v1.8.8: issue IBPB before switching to a new process's
                    // page table. This isolates the branch predictor state
                    // and mitigates Spectre v2 cross-process leakage.
                    crate::vendor::amd::cpu::zen3::errata_workarounds::issue_ibpb();
                    unsafe { crate::mm::virt::write_cr3(proc.page_table_root); }
                }
            }
        }

        task::set_current(next_idx);
    }
}

/// Yield the current task's remaining time slice.
pub fn yield_now() {
    if let Some(current) = task::current() {
        current.time_slice = 0;
        current.state = task::State::Ready;
    }
    schedule();
}

/// Initialize the scheduler. v1.7.5: no-op (tables live in BSS).
pub fn init() {
    // v2.0: configure quantum, priorities, runqueue.
    unsafe { TOTAL_TICKS = 0; IDLE_TICKS = 0; }
}

/// Public diagnostic counters. Read-only from other modules.
pub fn total_ticks() -> u64 { unsafe { TOTAL_TICKS } }
pub fn idle_ticks() -> u64   { unsafe { IDLE_TICKS } }

