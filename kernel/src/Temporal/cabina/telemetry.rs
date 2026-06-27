//! `cabina::telemetry` — Contadores atómicos en tiempo real.
//!
//! Lectura sin locks (solo `Ordering::Relaxed`). Escritura tampoco
//! usa locks (Atomic). Compatible con Ring 0 (snapshot vía cabina).
//!
//! ## Categorías
//!
//! - **CPU**: interrupts, faults, ticks
//! - **Memory**: allocs, frees, heap
//! - **Scheduler**: context switches, processes, threads
//! - **Syscall**: total + per-call counters
//! - **I/O**: PCI, serial, PS/2

#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, Ordering};

// ─── CPU ───────────────────────────────────────────────────────────

pub mod cpu {
    use super::*;

    static INTERRUPTS:    AtomicU64 = AtomicU64::new(0);
    static TIMER_TICKS:  AtomicU64 = AtomicU64::new(0);
    static PAGE_FAULTS:  AtomicU64 = AtomicU64::new(0);
    static GP_FAULTS:    AtomicU64 = AtomicU64::new(0);
    static NM_FAULTS:    AtomicU64 = AtomicU64::new(0);
    static DF_FAULTS:    AtomicU64 = AtomicU64::new(0);
    static UD_FAULTS:    AtomicU64 = AtomicU64::new(0);
    static MC_FAULTS:    AtomicU64 = AtomicU64::new(0);

    pub fn inc_interrupts() { INTERRUPTS.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_timer() { TIMER_TICKS.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_pf() { PAGE_FAULTS.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_gp() { GP_FAULTS.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_nm() { NM_FAULTS.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_df() { DF_FAULTS.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_ud() { UD_FAULTS.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_mc() { MC_FAULTS.fetch_add(1, Ordering::Relaxed); }

    pub fn get_interrupts() -> u64 { INTERRUPTS.load(Ordering::Relaxed) }
    pub fn get_timer() -> u64 { TIMER_TICKS.load(Ordering::Relaxed) }
    pub fn get_pf() -> u64 { PAGE_FAULTS.load(Ordering::Relaxed) }
    pub fn get_gp() -> u64 { GP_FAULTS.load(Ordering::Relaxed) }
    pub fn get_nm() -> u64 { NM_FAULTS.load(Ordering::Relaxed) }
    pub fn get_df() -> u64 { DF_FAULTS.load(Ordering::Relaxed) }
    pub fn get_ud() -> u64 { UD_FAULTS.load(Ordering::Relaxed) }
    pub fn get_mc() -> u64 { MC_FAULTS.load(Ordering::Relaxed) }
}

// ─── Memory ────────────────────────────────────────────────────────

pub mod memory {
    use super::*;
    static ALLOCS: AtomicU64 = AtomicU64::new(0);
    static FREES:  AtomicU64 = AtomicU64::new(0);
    static HEAP_USED:  AtomicU64 = AtomicU64::new(0);
    static HEAP_PEAK:  AtomicU64 = AtomicU64::new(0);
    static FREE_PAGES: AtomicU64 = AtomicU64::new(0);

    pub fn inc_allocs() { ALLOCS.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_frees() { FREES.fetch_add(1, Ordering::Relaxed); }
    pub fn add_heap_used(n: u64) {
        let v = HEAP_USED.fetch_add(n, Ordering::Relaxed) + n;
        let mut peak = HEAP_PEAK.load(Ordering::Relaxed);
        while v > peak {
            match HEAP_PEAK.compare_exchange(peak, v, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break,
                Err(p) => peak = p,
            }
        }
    }
    pub fn sub_heap_used(n: u64) { HEAP_USED.fetch_sub(n, Ordering::Relaxed); }
    pub fn set_free_pages(n: u64) { FREE_PAGES.store(n, Ordering::Relaxed); }

    pub fn get_allocs() -> u64 { ALLOCS.load(Ordering::Relaxed) }
    pub fn get_frees() -> u64 { FREES.load(Ordering::Relaxed) }
    pub fn get_heap_used() -> u64 { HEAP_USED.load(Ordering::Relaxed) }
    pub fn get_heap_peak() -> u64 { HEAP_PEAK.load(Ordering::Relaxed) }
    pub fn get_free_pages() -> u64 { FREE_PAGES.load(Ordering::Relaxed) }
}

// ─── Scheduler ─────────────────────────────────────────────────────

pub mod scheduler {
    use super::*;
    static CTX_SWITCHES: AtomicU64 = AtomicU64::new(0);
    static PROCESSES:    AtomicU64 = AtomicU64::new(0);
    static THREADS:      AtomicU64 = AtomicU64::new(0);

    pub fn inc_ctx() { CTX_SWITCHES.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_proc() { PROCESSES.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_thread() { THREADS.fetch_add(1, Ordering::Relaxed); }

    pub fn get_ctx() -> u64 { CTX_SWITCHES.load(Ordering::Relaxed) }
    pub fn get_proc() -> u64 { PROCESSES.load(Ordering::Relaxed) }
    pub fn get_thread() -> u64 { THREADS.load(Ordering::Relaxed) }
}

// ─── Syscall ───────────────────────────────────────────────────────

pub mod syscall {
    use super::*;
    use crate::bmo_abi::syscalls;

    static TOTAL: AtomicU64 = AtomicU64::new(0);
    /// Per-call counter, indexado por syscall number.
    static PER: [AtomicU64; 256] = [const { AtomicU64::new(0) }; 256];

    pub fn inc(nr: u16) {
        TOTAL.fetch_add(1, Ordering::Relaxed);
        if (nr as usize) < PER.len() {
            PER[nr as usize].fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn get_total() -> u64 { TOTAL.load(Ordering::Relaxed) }
    pub fn get_per(nr: u16) -> u64 {
        if (nr as usize) < PER.len() { PER[nr as usize].load(Ordering::Relaxed) } else { 0 }
    }

    /// Itera sobre los syscalls que tienen count > 0.
    pub fn iter_active() -> alloc::vec::Vec<(u16, u64)> {
        let mut out = alloc::vec::Vec::new();
        for i in 0..PER.len() {
            let c = PER[i].load(Ordering::Relaxed);
            if c > 0 {
                out.push((i as u16, c));
            }
        }
        out
    }

    /// Nombre del syscall nr (e.g. "win_create", "fs_open", "diag_print").
    /// v1.8.8: simplificación. Devuelve el nr como string.
    pub fn name(nr: u16) -> &'static str {
        let n = nr as u32;
        match n {
            x if x == syscalls::NR_WM_CREATE_WINDOW => "wm_create_window",
            x if x == syscalls::NR_WM_DESTROY_WINDOW => "wm_destroy_window",
            x if x == syscalls::NR_WM_SHOW_WINDOW => "wm_show_window",
            x if x == syscalls::NR_FS_OPEN => "fs_open",
            x if x == syscalls::NR_FS_CLOSE => "fs_close",
            x if x == syscalls::NR_FS_READ => "fs_read",
            x if x == syscalls::NR_FS_WRITE => "fs_write",
            x if x == syscalls::NR_TIME_NOW_NS => "time_now_ns",
            x if x == syscalls::NR_DEBUG_PRINT => "debug_print",
            x if x == syscalls::NR_PROC_EXIT => "proc_exit",
            x if x == syscalls::NR_BEFCORE_SEND => "befcore_send",
            x if x == syscalls::NR_BEFCORE_RECV => "befcore_recv",
            _ => "unknown",
        }
    }
}

// ─── I/O ───────────────────────────────────────────────────────────

pub mod io {
    use super::*;
    static PCI_READS:  AtomicU64 = AtomicU64::new(0);
    static PCI_WRITES: AtomicU64 = AtomicU64::new(0);
    static SERIAL_BYTES: AtomicU64 = AtomicU64::new(0);
    static PS2_SCANS:  AtomicU64 = AtomicU64::new(0);

    pub fn inc_pci_read()  { PCI_READS.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_pci_write() { PCI_WRITES.fetch_add(1, Ordering::Relaxed); }
    pub fn add_serial(n: u64) { SERIAL_BYTES.fetch_add(n, Ordering::Relaxed); }
    pub fn inc_ps2() { PS2_SCANS.fetch_add(1, Ordering::Relaxed); }

    pub fn get_pci_reads() -> u64 { PCI_READS.load(Ordering::Relaxed) }
    pub fn get_pci_writes() -> u64 { PCI_WRITES.load(Ordering::Relaxed) }
    pub fn get_serial() -> u64 { SERIAL_BYTES.load(Ordering::Relaxed) }
    pub fn get_ps2() -> u64 { PS2_SCANS.load(Ordering::Relaxed) }
    pub fn get_ps2_scans() -> u64 { PS2_SCANS.load(Ordering::Relaxed) }
}

/// Inicializa los contadores.
pub fn init() {
    // Nada que hacer — los Atomic se inicializan a 0 por default.
}
