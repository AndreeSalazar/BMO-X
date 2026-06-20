//! Scheduler de FastOS — Round-Robin con prioridades.
//!
//! Diseñado para Ryzen 5 5600X (1 CCD × 6 cores × 2 threads).
//! Integra con APIC timer para preemptive scheduling.
//!
//! Modular:
//!   - process: Process management
//!   - thread: Thread management + context switching
//!   - rt: Real-time scheduler (EDF) — reserved, no active callers
//!   - user_init: Ring 3 process loading (spawn_hello para el welcome)

#![allow(dead_code)]

pub mod process;
pub mod thread;
pub mod user_init;
pub mod rt;

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
    /// Cores físicos solamente (sin SMT) — mejor para threads sensibles a latencia.
    pub const PHYSICAL_ONLY: Self = Self { mask: 0b0000_0101_0101_0101 };
}

/// Called from APIC timer interrupt — performs preemptive scheduling.
///
/// `saved_rsp` is the RSP pointing to the interrupted context saved on the kernel stack.
pub fn timer_tick() {
    // Decrement time slice of current thread
    if let Some(current) = thread::current_thread() {
        if current.time_slice > 0 {
            current.time_slice -= 1;
        }
        if current.time_slice == 0 {
            // Time's up — mark as Ready and pick next
            current.state = thread::ThreadState::Ready;
            schedule();
        }
    }
}

/// Pick next thread and switch to it.
pub fn schedule() {
    let current_idx = thread::current_index();

    if let Some(next_idx) = thread::pick_next() {
        if next_idx == current_idx {
            // Same thread, just reset slice
            if let Some(t) = thread::get_thread(next_idx) {
                t.state = thread::ThreadState::Running;
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

        // Mark current as Ready (if still Running)
        if let Some(cur) = thread::get_thread(current_idx) {
            if cur.state == thread::ThreadState::Running {
                cur.state = thread::ThreadState::Ready;
            }
        }

        // Switch to next
        if let Some(next) = thread::get_thread(next_idx) {
            next.state = thread::ThreadState::Running;
            next.time_slice = match next.priority {
                Priority::Realtime => 1,
                Priority::HighGame => 5,
                Priority::Game => 10,
                Priority::Interactive => 20,
                Priority::Idle => 50,
            };

            // Update kernel stack for syscalls from this thread
            crate::interrupt::gdt::set_kernel_stack(next.kernel_stack_top);
            crate::interrupt::syscall::set_syscall_kernel_stack(next.kernel_stack_top);

            // Switch CR3 if process has different page table
            if let Some(proc) = process::get_process(next.pid) {
                if proc.page_table_root != 0 {
                    let current_cr3 = crate::memory::paging::read_cr3();
                    if proc.page_table_root != current_cr3 {
                        crate::bmo_core::diag::trace_u64("sched", "CR3 switch", proc.page_table_root);
                        unsafe { crate::memory::paging::write_cr3(proc.page_table_root); }
                    }
                }
            }

            thread::set_current(next_idx);

            // Telemetry: context switch
            crate::bmo_core::diag::telemetry::t().sched.record_context_switch();
        }
    }
}

/// Yield the current thread's remaining time slice.
pub fn yield_now() {
    if let Some(current) = thread::current_thread() {
        current.time_slice = 0;
        current.state = thread::ThreadState::Ready;
    }
    schedule();
}

/// Inicializa el scheduler. v1.7.4: no-op (las tablas viven en BSS).
/// Se llama desde `ring_0::ring_0::init()` después de crate::interrupt::apic::init.
pub fn init() {
    // v2.0: configurar quantum, prioridades, runqueue.
}
