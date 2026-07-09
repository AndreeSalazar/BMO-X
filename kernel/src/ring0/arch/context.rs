//! BootContext â€” Dependency Injection container for boot phases.
//!
//! v1.1.0: Replaces scattered globals with a single, typed ctx that
//!
//! v1.6.16: `tsc_start`/`tsc_end` fields and `elapsed_tsc` method
//! are reserved for the per-phase timing dashboard in v1.7.x.

//! each phase receives explicitly. This makes phase dependencies
//! auditable and testable.
//!
//! ## Migration map
//!
//! Old (globals)                â†’ New (ctx field)
//! `crate::mm::phys::free_count` â†’ `ctx.memory.free_pages`
//! `crate::cpu::tsc_per_sec`     â†’ `ctx.cpu.tsc_freq_hz`
//! `bmo_abi clock init`          â†’ `ctx.bmo_abi_initialized`
//! `crate::mm::heap::heap_total`      â†’ `ctx.memory.heap_total`
//!
//! ## Stability
//!
//! All fields are `pub` for now (no accessor pattern) to avoid
//! breaking the many callsites in one go. v1.2.0 will introduce
//! accessors and make fields private.

use bmo_boot_protocol::BootInfo;

/// Aggregate of CPU-related state captured during Phase 0.
#[derive(Clone, Copy)]
pub struct CpuContext {
    pub tsc_freq_hz: u64,
    pub vendor: [u8; 12],
    pub features_sse: bool,
    pub features_avx: bool,
    pub features_avx2: bool,
    pub features_aes: bool,
}

impl CpuContext {
    pub const fn empty() -> Self {
        Self {
            tsc_freq_hz: 0,
            vendor: [0; 12],
            features_sse: false,
            features_avx: false,
            features_avx2: false,
            features_aes: false,
        }
    }
}

/// Aggregate of memory-related state captured during Phase 1.
#[derive(Clone, Copy)]
pub struct MemoryContext {
    pub free_pages: u64,
    pub free_mb: u64,
    pub heap_total_bytes: u64,
    pub heap_used_bytes: u64,
}

impl MemoryContext {
    pub const fn empty() -> Self {
        Self {
            free_pages: 0,
            free_mb: 0,
            heap_total_bytes: 0,
            heap_used_bytes: 0,
        }
    }
}

/// Aggregate of device-scan state captured during Phase 2.
#[derive(Clone, Copy, Default)]
pub struct DevicesContext {
    pub acpi_mcfg_base: u64,
    pub acpi_mcfg_end_bus: u8,
    pub pci_devices_found: u32,
    pub ecam_mapped: bool,
}

impl DevicesContext {
    pub const fn empty() -> Self {
        Self {
            acpi_mcfg_base: 0,
            acpi_mcfg_end_bus: 0,
            pci_devices_found: 0,
            ecam_mapped: false,
        }
    }
}

/// The full boot ctx. Each phase receives `&mut BootContext` and
/// reads/writes the slice it owns. Once a phase is done, downstream
/// phases can rely on the data being filled in.
///
/// v1.5.1: `boot_info` was a `BootInfo` by value (4.2 KB on stack,
/// caused overflow). Now it's a `*const BootInfo` pointing to the
/// bootloader's memory, which is identity-mapped and persistent.
pub struct BootContext {
    pub boot_info: *const BootInfo,
    pub cpu: CpuContext,
    pub memory: MemoryContext,
    pub devices: DevicesContext,
    pub phase_outputs: [Option<PhaseSnapshot>; 8],
}

/// Lightweight snapshot of one phase's output â€” useful for cross-phase
/// introspection (e.g. "did Phase 1 actually set up the heap?").
#[derive(Clone, Copy)]
pub struct PhaseSnapshot {
    pub tsc_end: u64,
    pub tsc_start: u64,
}

impl BootContext {
    /// Create a BootContext pointing to the bootloader's BootInfo.
    /// v1.5.1: takes a pointer instead of a value to avoid 4.2 KB stack copy.
    pub fn new(boot_info: *const BootInfo) -> Self {
        Self {
            boot_info,
            cpu: CpuContext::empty(),
            memory: MemoryContext::empty(),
            devices: DevicesContext::empty(),
            phase_outputs: [None; 8],
        }
    }

    /// Returns the BootInfo pointer. Returns None if the pointer is null.
    pub fn boot_info(&self) -> Option<*const BootInfo> {
        if self.boot_info.is_null() {
            None
        } else {
            Some(self.boot_info as *const BootInfo)
        }
    }

    /// Record a phase's timing. `phase_index` is the phase's number
    /// (0 for Phase 0 CPU, 1 for Phase 1 Memory, ...).
    pub fn record_phase(&mut self, phase_index: usize, tsc_start: u64, tsc_end: u64) {
        if phase_index < self.phase_outputs.len() {
            self.phase_outputs[phase_index] = Some(PhaseSnapshot { tsc_start, tsc_end });
        }
    }
}
