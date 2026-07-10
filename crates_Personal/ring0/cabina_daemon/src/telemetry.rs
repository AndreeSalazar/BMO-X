use core::sync::atomic::{AtomicU64, Ordering};
use cabina_core::{CpuCounters, MemoryCounters, SchedulerCounters, IoCounters};

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

    pub fn snapshot() -> CpuCounters {
        CpuCounters {
            interrupts: INTERRUPTS.load(Ordering::Relaxed),
            timer_ticks: TIMER_TICKS.load(Ordering::Relaxed),
            page_faults: PAGE_FAULTS.load(Ordering::Relaxed),
            general_protection: GP_FAULTS.load(Ordering::Relaxed),
            nmi: NM_FAULTS.load(Ordering::Relaxed),
            double_fault: DF_FAULTS.load(Ordering::Relaxed),
            undefined_opcode: UD_FAULTS.load(Ordering::Relaxed),
            machine_check: MC_FAULTS.load(Ordering::Relaxed),
        }
    }
}

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

    pub fn snapshot() -> MemoryCounters {
        MemoryCounters {
            allocations: ALLOCS.load(Ordering::Relaxed),
            frees: FREES.load(Ordering::Relaxed),
            heap_used: HEAP_USED.load(Ordering::Relaxed),
            heap_peak: HEAP_PEAK.load(Ordering::Relaxed),
            free_pages: FREE_PAGES.load(Ordering::Relaxed),
        }
    }
}

pub mod scheduler {
    use super::*;
    static CTX_SWITCHES: AtomicU64 = AtomicU64::new(0);
    static PROCESSES:    AtomicU64 = AtomicU64::new(0);
    static THREADS:      AtomicU64 = AtomicU64::new(0);

    pub fn inc_ctx() { CTX_SWITCHES.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_proc() { PROCESSES.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_thread() { THREADS.fetch_add(1, Ordering::Relaxed); }

    pub fn snapshot() -> SchedulerCounters {
        SchedulerCounters {
            context_switches: CTX_SWITCHES.load(Ordering::Relaxed),
            processes: PROCESSES.load(Ordering::Relaxed),
            threads: THREADS.load(Ordering::Relaxed),
        }
    }
}

pub mod syscall {
    use super::*;
    static TOTAL: AtomicU64 = AtomicU64::new(0);
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

    pub fn snapshot() -> [u64; 256] {
        let mut out = [0u64; 256];
        for (i, slot) in PER.iter().enumerate() {
            out[i] = slot.load(Ordering::Relaxed);
        }
        out
    }
}

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

    pub fn snapshot() -> IoCounters {
        IoCounters {
            pci_reads: PCI_READS.load(Ordering::Relaxed),
            pci_writes: PCI_WRITES.load(Ordering::Relaxed),
            serial_bytes: SERIAL_BYTES.load(Ordering::Relaxed),
            ps2_scancodes: PS2_SCANS.load(Ordering::Relaxed),
        }
    }
}

pub fn snapshot() -> cabina_core::TelemetrySnapshot {
    cabina_core::TelemetrySnapshot {
        uptime_ns: 0,
        cpu: cpu::snapshot(),
        memory: memory::snapshot(),
        scheduler: scheduler::snapshot(),
        io: io::snapshot(),
        syscall_counts: syscall::snapshot(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_snapshot() {
        cpu::inc_interrupts();
        cpu::inc_timer();
        cpu::inc_pf();
        let s = cpu::snapshot();
        assert_eq!(s.interrupts, 1);
        assert_eq!(s.timer_ticks, 1);
        assert_eq!(s.page_faults, 1);
    }

    #[test]
    fn memory_snapshot() {
        memory::inc_allocs();
        memory::add_heap_used(4096);
        memory::inc_frees();
        let s = memory::snapshot();
        assert_eq!(s.allocations, 1);
        assert_eq!(s.heap_used, 4096);
        assert_eq!(s.frees, 1);
    }

    #[test]
    fn scheduler_snapshot() {
        scheduler::inc_ctx();
        scheduler::inc_proc();
        scheduler::inc_thread();
        let s = scheduler::snapshot();
        assert_eq!(s.context_switches, 1);
    }

    #[test]
    fn io_snapshot() {
        io::inc_pci_read();
        io::add_serial(64);
        let s = io::snapshot();
        assert_eq!(s.pci_reads, 1);
        assert_eq!(s.serial_bytes, 64);
    }

    #[test]
    fn syscall_counters() {
        syscall::inc(42);
        syscall::inc(42);
        syscall::inc(7);
        assert_eq!(syscall::get_total(), 3);
        assert_eq!(syscall::get_per(42), 2);
        assert_eq!(syscall::get_per(7), 1);
        let active = syscall::iter_active();
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn telemetry_snapshot_aggregates() {
        cpu::inc_interrupts();
        memory::inc_allocs();
        scheduler::inc_ctx();
        io::inc_pci_read();
        let s = snapshot();
        assert!(s.cpu.interrupts >= 1);
        assert!(s.memory.allocations >= 1);
        assert!(s.scheduler.context_switches >= 1);
        assert!(s.io.pci_reads >= 1);
    }
}
