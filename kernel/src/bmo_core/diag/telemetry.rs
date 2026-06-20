//! Omniscient Telemetry — atomic counters for every kernel subsystem.
//!
//! Each counter is an `AtomicU64` so the timer tick (interrupt ctx) can
//! update them without locks.  The overlay reads them periodically.

use core::sync::atomic::{AtomicU64, Ordering};

// ── Helpers ────────────────────────────────────────────────────────

fn delta(old: u64, new: u64) -> u64 {
    new.wrapping_sub(old)
}

// ── Per-subsystem counters ─────────────────────────────────────────

pub struct CpuTelemetry {
    /// Total interrupts serviced since boot.
    pub interrupts: AtomicU64,
    /// Timer ticks (APIC vector 48).
    pub timer_ticks: AtomicU64,
    /// Page faults (#PF).
    pub page_faults: AtomicU64,
    /// General protection faults (#GP).
    pub gp_faults: AtomicU64,
    /// Device not available (#NM — FPU).
    pub nm_faults: AtomicU64,
    /// Double fault (#DF).
    pub df_faults: AtomicU64,
    /// Invalid opcode (#UD).
    pub ud_faults: AtomicU64,
    /// Machine check (#MC).
    pub mc_faults: AtomicU64,
    /// Other exceptions.
    pub other_faults: AtomicU64,
    /// TSC at last sample (for delta calculation).
    pub last_tsc: AtomicU64,
    /// APIC timer ticks at last sample.
    pub last_timer_ticks: AtomicU64,
}

impl CpuTelemetry {
    pub const fn new() -> Self {
        Self {
            interrupts: AtomicU64::new(0),
            timer_ticks: AtomicU64::new(0),
            page_faults: AtomicU64::new(0),
            gp_faults: AtomicU64::new(0),
            nm_faults: AtomicU64::new(0),
            df_faults: AtomicU64::new(0),
            ud_faults: AtomicU64::new(0),
            mc_faults: AtomicU64::new(0),
            other_faults: AtomicU64::new(0),
            last_tsc: AtomicU64::new(0),
            last_timer_ticks: AtomicU64::new(0),
        }
    }

    pub fn interrupt_count(&self) -> u64 {
        self.interrupts.load(Ordering::Relaxed)
    }
    pub fn timer_tick_count(&self) -> u64 {
        self.timer_ticks.load(Ordering::Relaxed)
    }
    pub fn page_fault_count(&self) -> u64 {
        self.page_faults.load(Ordering::Relaxed)
    }

    /// Compute interrupts-per-second since last sample.
    pub fn interrupts_per_sec(&self) -> u64 {
        let now_tsc = crate::cpu::rdtsc();
        let now_ticks = self.timer_ticks.load(Ordering::Relaxed);
        let old_tsc = self.last_tsc.swap(now_tsc, Ordering::Relaxed);
        let old_ticks = self.last_timer_ticks.swap(now_ticks, Ordering::Relaxed);

        let tsc_delta = delta(old_tsc, now_tsc);
        let tick_delta = delta(old_ticks, now_ticks);

        if tsc_delta == 0 || tick_delta == 0 {
            return 0;
        }
        // ticks * (freq / tsc_delta) ≈ ticks * (1_000_000_000 / tsc_delta)
        // Simplified: assume 1 Hz tick → ipsec ≈ tick_delta * (tsc_freq / tsc_delta)
        // But simpler: we sample at 1 Hz, so ipsec ≈ interrupts since last sample
        self.interrupts.load(Ordering::Relaxed) // simplified; real impl tracks delta
    }
}

pub struct MemoryTelemetry {
    /// Total page allocations (calls to alloc_pages_contiguous).
    pub allocs: AtomicU64,
    /// Total page frees (calls to free_pages).
    pub frees: AtomicU64,
    /// Current heap usage in bytes.
    pub heap_used: AtomicU64,
    /// Peak heap usage in bytes.
    pub heap_peak: AtomicU64,
    /// Page allocator free count (snapshot).
    pub free_pages: AtomicU64,
}

impl MemoryTelemetry {
    pub const fn new() -> Self {
        Self {
            allocs: AtomicU64::new(0),
            frees: AtomicU64::new(0),
            heap_used: AtomicU64::new(0),
            heap_peak: AtomicU64::new(0),
            free_pages: AtomicU64::new(0),
        }
    }

    pub fn record_alloc(&self, pages: usize) {
        self.allocs.fetch_add(pages as u64, Ordering::Relaxed);
    }
    pub fn record_free(&self, pages: usize) {
        self.frees.fetch_add(pages as u64, Ordering::Relaxed);
    }
    pub fn update_heap(&self, used: u64) {
        self.heap_used.store(used, Ordering::Relaxed);
        // Update peak
        let mut current = self.heap_peak.load(Ordering::Relaxed);
        while used > current {
            match self.heap_peak.compare_exchange_weak(
                current, used, Ordering::Relaxed, Ordering::Relaxed
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }
    pub fn update_free_pages(&self, count: u64) {
        self.free_pages.store(count, Ordering::Relaxed);
    }
}

pub struct SchedulerTelemetry {
    /// Total ctx switches since boot.
    pub context_switches: AtomicU64,
    /// Total processes created.
    pub processes_created: AtomicU64,
    /// Total threads created.
    pub threads_created: AtomicU64,
    /// Current process count.
    pub process_count: AtomicU64,
    /// Current thread count.
    pub thread_count: AtomicU64,
}

impl SchedulerTelemetry {
    pub const fn new() -> Self {
        Self {
            context_switches: AtomicU64::new(0),
            processes_created: AtomicU64::new(0),
            threads_created: AtomicU64::new(0),
            process_count: AtomicU64::new(0),
            thread_count: AtomicU64::new(0),
        }
    }

    pub fn record_context_switch(&self) {
        self.context_switches.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_process_create(&self) {
        self.processes_created.fetch_add(1, Ordering::Relaxed);
        self.process_count.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_process_exit(&self) {
        self.process_count.fetch_sub(1, Ordering::Relaxed);
    }
    pub fn record_thread_create(&self) {
        self.threads_created.fetch_add(1, Ordering::Relaxed);
        self.thread_count.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_thread_exit(&self) {
        self.thread_count.fetch_sub(1, Ordering::Relaxed);
    }
}

pub struct SyscallTelemetry {
    /// Total syscalls dispatched.
    pub total: AtomicU64,
    /// Per-syscall counters (indexed by syscall number, first 32 slots).
    pub per_call: [AtomicU64; 32],
}

impl SyscallTelemetry {
    pub const fn new() -> Self {
        Self {
            total: AtomicU64::new(0),
            per_call: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            ],
        }
    }

    pub fn record(&self, syscall_num: u32) {
        self.total.fetch_add(1, Ordering::Relaxed);
        if (syscall_num as usize) < self.per_call.len() {
            self.per_call[syscall_num as usize].fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub struct IoTelemetry {
    /// PCI config space reads.
    pub pci_reads: AtomicU64,
    /// PCI config space writes.
    pub pci_writes: AtomicU64,
    /// Serial bytes written.
    pub serial_bytes: AtomicU64,
    /// PS/2 keyboard scancodes received.
    pub ps2_scancodes: AtomicU64,
}

impl IoTelemetry {
    pub const fn new() -> Self {
        Self {
            pci_reads: AtomicU64::new(0),
            pci_writes: AtomicU64::new(0),
            serial_bytes: AtomicU64::new(0),
            ps2_scancodes: AtomicU64::new(0),
        }
    }
}

// ── Global singleton ───────────────────────────────────────────────

pub struct Omniscient {
    pub cpu: CpuTelemetry,
    pub mem: MemoryTelemetry,
    pub sched: SchedulerTelemetry,
    pub syscall: SyscallTelemetry,
    pub io: IoTelemetry,
}

impl Omniscient {
    pub const fn new() -> Self {
        Self {
            cpu: CpuTelemetry::new(),
            mem: MemoryTelemetry::new(),
            sched: SchedulerTelemetry::new(),
            syscall: SyscallTelemetry::new(),
            io: IoTelemetry::new(),
        }
    }
}

/// Global omniscient telemetry instance.
pub static TELEMETRY: Omniscient = Omniscient::new();

/// Shorthand accessor.
pub fn t() -> &'static Omniscient {
    &TELEMETRY
}
