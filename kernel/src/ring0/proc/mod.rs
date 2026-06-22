//! Scheduler de FastOS — Round-Robin con prioridades.
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

#![allow(dead_code)]

pub mod process;
pub mod task;
pub mod user_init;

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

/// Called from APIC timer interrupt — performs preemptive scheduling.
pub fn timer_tick() {
    if let Some(current) = task::current() {
        if current.time_slice > 0 {
            current.time_slice -= 1;
        }
        if current.time_slice == 0 {
            current.state = task::State::Ready;
            schedule();
        }
    }
}

/// Pick next task and switch to it.
pub fn schedule() {
    let current_idx = task::current_index();

    if let Some(next_idx) = task::pick_next() {
        if next_idx == current_idx {
            if let Some(t) = task::get(next_idx) {
                t.state = task::State::Running;
                t.time_slice = match t.priority {
                    Priority::Realtime => 1,
                    Priority::HighGame => 5,
                    Priority::Game => 10,
                    Priority::Interactive => 20,
                    Priority::Idle => 50,
                };
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
            next.time_slice = match next.priority {
                Priority::Realtime => 1,
                Priority::HighGame => 5,
                Priority::Game => 10,
                Priority::Interactive => 20,
                Priority::Idle => 50,
            };

            crate::arch::gdt::set_kernel_stack(next.kernel_stack_top);
            crate::arch::syscall::set_syscall_kernel_stack(next.kernel_stack_top);

            if let Some(proc) = process::get_process(next.pid) {
                if proc.page_table_root != 0 {
                    let current_cr3 = crate::mem::virt::read_cr3();
                    if proc.page_table_root != current_cr3 {
                        crate::cabina::trace_u64("sched", "CR3 switch", proc.page_table_root);
                        // v1.8.8: issue IBPB before switching to a new process's
                        // page table. This isolates the branch predictor state
                        // and mitigates Spectre v2 cross-process leakage.
                        crate::vendor::amd::cpu::zen3::errata_workarounds::issue_ibpb();
                        unsafe { crate::mem::virt::write_cr3(proc.page_table_root); }
                    }
                }
            }

            task::set_current(next_idx);
            crate::bmo_core::diag::telemetry::t().sched.record_context_switch();
        }
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
}



