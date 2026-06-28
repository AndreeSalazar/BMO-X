use crate::event::Event;

// ─── Counter group: snapshot of all atomic counters at a point in time ────

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuCounters {
    pub interrupts: u64,
    pub timer_ticks: u64,
    pub page_faults: u64,
    pub general_protection: u64,
    pub nmi: u64,
    pub double_fault: u64,
    pub undefined_opcode: u64,
    pub machine_check: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryCounters {
    pub allocations: u64,
    pub frees: u64,
    pub heap_used: u64,
    pub heap_peak: u64,
    pub free_pages: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerCounters {
    pub context_switches: u64,
    pub processes: u64,
    pub threads: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoCounters {
    pub pci_reads: u64,
    pub pci_writes: u64,
    pub serial_bytes: u64,
    pub ps2_scancodes: u64,
}

// ─── Full system telemetry snapshot ───────────────────────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TelemetrySnapshot {
    pub uptime_ns: u64,
    pub cpu: CpuCounters,
    pub memory: MemoryCounters,
    pub scheduler: SchedulerCounters,
    pub io: IoCounters,
    /// Per-syscall counters (indexed by syscall number, 0 = total).
    pub syscall_counts: [u64; 256],
}

impl TelemetrySnapshot {
    pub const fn zero() -> Self {
        Self {
            uptime_ns: 0,
            cpu: CpuCounters {
                interrupts: 0, timer_ticks: 0, page_faults: 0,
                general_protection: 0, nmi: 0, double_fault: 0,
                undefined_opcode: 0, machine_check: 0,
            },
            memory: MemoryCounters {
                allocations: 0, frees: 0, heap_used: 0, heap_peak: 0, free_pages: 0,
            },
            scheduler: SchedulerCounters {
                context_switches: 0, processes: 0, threads: 0,
            },
            io: IoCounters {
                pci_reads: 0, pci_writes: 0, serial_bytes: 0, ps2_scancodes: 0,
            },
            syscall_counts: [0u64; 256],
        }
    }
}

// ─── Full CABINA snapshot: telemetry + recent events ──────────────────────

pub const SNAPSHOT_EVENTS_MAX: usize = 32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SystemSnapshot {
    pub telemetry: TelemetrySnapshot,
    pub event_count: u32,
    pub events: [Event; SNAPSHOT_EVENTS_MAX],
}

impl SystemSnapshot {
    pub const fn zero() -> Self {
        Self {
            telemetry: TelemetrySnapshot::zero(),
            event_count: 0,
            events: [Event::ZERO; SNAPSHOT_EVENTS_MAX],
        }
    }
}

// ─── Event in Event needs a ZERO constant ────────────────────────────────

impl Event {
    pub const ZERO: Self = Self {
        seq: 0,
        tick_ns: 0,
        severity: crate::event::Severity::Info,
        layer: crate::event::Layer::Ring0,
        entity: crate::event::Entity::Module,
        _pad: 0,
        module: [0u8; crate::event::MODULE_MAX],
        entity_id: 0,
        msg: [0u8; crate::event::MSG_MAX],
        value: 0,
    };
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_snapshot_zero() {
        let s = TelemetrySnapshot::zero();
        assert_eq!(s.uptime_ns, 0);
        assert_eq!(s.cpu.interrupts, 0);
        assert_eq!(s.memory.allocations, 0);
        assert_eq!(s.scheduler.context_switches, 0);
        assert_eq!(s.io.pci_reads, 0);
    }

    #[test]
    fn telemetry_copy_works() {
        let mut a = TelemetrySnapshot::zero();
        a.cpu.interrupts = 42;
        let b = a;
        assert_eq!(b.cpu.interrupts, 42);
    }

    #[test]
    fn system_snapshot_zero() {
        let s = SystemSnapshot::zero();
        assert_eq!(s.event_count, 0);
        assert_eq!(s.events.len(), SNAPSHOT_EVENTS_MAX);
    }

    #[test]
    fn system_snapshot_holds_events() {
        let mut s = SystemSnapshot::zero();
        s.event_count = 2;
        s.events[0] = Event::new(
            crate::event::Severity::Info,
            crate::event::Layer::Ring0,
            crate::event::Entity::Module,
            "test", 0, "snapshot", 0,
        );
        s.events[1] = Event::new(
            crate::event::Severity::Fault,
            crate::event::Layer::BmoCore,
            crate::event::Entity::Syscall,
            "vm", 1, "crash", 0xBAD,
        );
        assert_eq!(s.events[0].module_str(), "test");
        assert_eq!(s.events[1].module_str(), "vm");
    }

    #[test]
    fn event_zero_const() {
        let e = Event::ZERO;
        assert_eq!(e.seq, 0);
        assert_eq!(e.module_str(), "");
        assert_eq!(e.msg_str(), "");
    }

    #[test]
    fn repr_c_layout() {
        use core::mem;
        // TelemetrySnapshot should be large enough for 256 u64 counters
        assert_eq!(mem::size_of::<TelemetrySnapshot>(), 8 + 8*8 + 8*5 + 8*3 + 8*4 + 8*256);
    }
}
