//! `cabina::snapshot` — API limpia para leer el estado del sistema.
//!
//! Es un **snapshot inmutable** del estado del sistema. Se usa
//! desde el overlay (HUD) y desde Ring 3 (apps que quieren saber
//! el estado del kernel).
//!
//! ## Uso
//!
//! ```ignore
//! let s = cabina::snapshot::take();
//! cabina::overlay::paint(&s, tab);
//! ```

#![allow(dead_code)]

use crate::cabina::telemetry;

/// Snapshot inmutable del estado del sistema.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub cpu: CpuSnapshot,
    pub memory: MemorySnapshot,
    pub scheduler: SchedulerSnapshot,
    pub syscalls: alloc::vec::Vec<(u16, u64)>,
    pub io: IoSnapshot,
    pub uptime_ns: u64,
    pub last_events: alloc::vec::Vec<crate::cabina::event::Event>,
}

#[derive(Clone, Debug, Default)]
pub struct CpuSnapshot {
    pub interrupts: u64,
    pub timer_ticks: u64,
    pub pf: u64,
    pub gp: u64,
    pub nm: u64,
    pub df: u64,
    pub ud: u64,
    pub mc: u64,
}

#[derive(Clone, Debug, Default)]
pub struct MemorySnapshot {
    pub allocs: u64,
    pub frees: u64,
    pub heap_used: u64,
    pub heap_peak: u64,
    pub free_pages: u64,
}

#[derive(Clone, Debug, Default)]
pub struct SchedulerSnapshot {
    pub ctx_switches: u64,
    pub processes: u64,
    pub threads: u64,
}

#[derive(Clone, Debug, Default)]
pub struct IoSnapshot {
    pub pci_reads: u64,
    pub pci_writes: u64,
    pub serial_bytes: u64,
    pub ps2_scans: u64,
}

/// Toma un snapshot del estado actual. Operación barata (solo Atomic loads).
pub fn take() -> Snapshot {
    Snapshot {
        cpu: CpuSnapshot {
            interrupts: telemetry::cpu::get_interrupts(),
            timer_ticks: telemetry::cpu::get_timer(),
            pf: telemetry::cpu::get_pf(),
            gp: telemetry::cpu::get_gp(),
            nm: telemetry::cpu::get_nm(),
            df: telemetry::cpu::get_df(),
            ud: telemetry::cpu::get_ud(),
            mc: telemetry::cpu::get_mc(),
        },
        memory: MemorySnapshot {
            allocs: telemetry::memory::get_allocs(),
            frees: telemetry::memory::get_frees(),
            heap_used: telemetry::memory::get_heap_used(),
            heap_peak: telemetry::memory::get_heap_peak(),
            free_pages: telemetry::memory::get_free_pages(),
        },
        scheduler: SchedulerSnapshot {
            ctx_switches: telemetry::scheduler::get_ctx(),
            processes: telemetry::scheduler::get_proc(),
            threads: telemetry::scheduler::get_thread(),
        },
        syscalls: telemetry::syscall::iter_active(),
        io: IoSnapshot {
            pci_reads: telemetry::io::get_pci_reads(),
            pci_writes: telemetry::io::get_pci_writes(),
            serial_bytes: telemetry::io::get_serial(),
            ps2_scans: telemetry::io::get_ps2_scans(),
        },
        uptime_ns: {
            let tsc = crate::cpu::rdtsc();
            let freq = crate::cpu::tsc_per_sec();
            if freq == 0 { 0 } else { tsc.wrapping_mul(1_000_000_000) / freq }
        },
        last_events: crate::cabina::event::buffer::last(32),
    }
}

