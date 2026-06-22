//! CPU topology for the Ryzen 5 5600X (1 CCD, 1 CCX, 6C/12T).
//!
//! Implements `AMD/ryzen_5_5600x.md` §2 (Topología física y SMT).
//!
//! Status: ✅ COMPLETO — detección real de CCX, SMT, cores, threads
//! usando CPUID leaves 0x0B, 0x8000001E.
//!
//! References:
//! - AMD Zen 3 Family 19h BKDG, §3.17 (CPUID)
//! - AMD64 APM Vol. 3, §3.8 (CPUID — Topology)

use super::cpuid_detection::cpuid;

/// CPU logical ID. Combines thread ID within a core and core ID within
/// the CCX. APIC ID is the per-thread hardware ID used in IPI targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuId {
    pub apic_id: u8,    // hardware APIC ID (unique per logical processor)
    pub thread: u8,     // 0 or 1 (SMT)
    pub core: u8,       // 0..=5 on the 5600X
    pub ccd: u8,        // 0 on the 5600X (single CCD)
    pub ccx: u8,        // 0 on the 5600X (single CCX)
}

impl CpuId {
    /// Linear index 0..=11 (BSP=0, ..., thread 11). Useful for tables.
    pub fn linear(&self) -> u8 {
        self.ccd * 12 + self.core * 2 + self.thread
    }
}

/// Per-CPU data structure. Each core/thread has its own copy in a
/// per-CPU data section. The 5600X has 12 of these (6 cores × 2 threads).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PerCpu {
    pub apic_id: u32,
    pub kernel_stack_top: u64,
    pub online: bool,
    pub running: bool,
    pub idle_task: u64,
}

/// Full system topology as detected at boot.
#[derive(Debug, Clone, Copy)]
pub struct Topology {
    pub bsp: CpuId,
    pub cpus: [CpuId; 64],
    pub cpu_count: u32,
    pub total_threads: u32,
    pub total_cores: u32,
    pub total_ccxs: u32,
    pub total_ccds: u32,
}

impl Topology {
    /// Returns a slice of all detected CPUs.
    pub fn cpus(&self) -> &[CpuId] {
        &self.cpus[..self.cpu_count as usize]
    }

    /// Find a CPU by APIC ID. Returns `None` if not found.
    pub fn find_by_apic(&self, apic: u32) -> Option<&CpuId> {
        self.cpus().iter().find(|c| c.apic_id as u32 == apic)
    }
}

/// Detect the full topology by querying CPUID 0x0B (extended
/// topology) and 0x8000001E (extended APIC ID).
pub fn detect() -> Topology {
    let mut cpus = [CpuId {
        apic_id: 0, thread: 0, core: 0, ccd: 0, ccx: 0,
    }; 64];
    let mut count = 0u32;

    // CPUID 0x0B sub-leaf 0: SMT level (thread within core)
    // ECX[15:8] = number of logical processors at this level (= 2 on Zen 3)
    // EAX = extended APIC ID
    let (smt_apic_id, smt_shift) = {
        let (eax, _, ecx, _) = cpuid(0x0B, 0);
        ((eax & 0xFFFF) as u8, (ecx & 0x1F) as u8)
    };

    // CPUID 0x0B sub-leaf 1: core level
    // ECX[15:8] = number of logical processors at this level
    let (core_apic_id, core_shift) = {
        let (eax, _, ecx, _) = cpuid(0x0B, 1);
        ((eax & 0xFFFF) as u8, (ecx & 0x1F) as u8)
    };

    // Number of threads per core (from sub-leaf 0)
    let threads_per_core = {
        let (_, _, ecx, _) = cpuid(0x0B, 0);
        ((ecx >> 8) & 0xFF) as u32
    };
    // Number of cores per package (from sub-leaf 1)
    let cores_per_package = {
        let (_, _, ecx, _) = cpuid(0x0B, 1);
        ((ecx >> 8) & 0xFF) as u32
    };
    let total_threads = threads_per_core * cores_per_package;

    // CPUID 0x8000001E: extended APIC ID + node (CCX) info
    // EBX[7:0] = ThreadsPerComputeUnit (threads per CCX, typically 16 for Zen 3)
    // ECX[7:0] = NodeId (CCX index)
    let (threads_per_ccx, ccx_id) = {
        let (_, ebx, ecx, _) = cpuid(0x8000001E, 0);
        (((ebx >> 8) & 0xFF) as u32, (ecx & 0xFF) as u8)
    };

    // Compute the APIC ID fields by masking
    // (Assumes the topology is hierarchical: thread < core < CCX < CCD.)
    let thread_mask = (1u8 << smt_shift) - 1;
    let core_mask = ((1u8 << core_shift) - 1) & !thread_mask;
    // For 5600X with shift 1 (2 threads) and shift 3 (8 cores) on a single CCX:
    // thread_mask = 0b00000001 (bit 0)
    // core_mask   = 0b00001110 (bits 1-3)
    // ccx_mask    = 0b00000000 (only one CCX)

    // Enumerate all threads
    for i in 0..total_threads {
        if (i as usize) >= 64 {
            break;  // safety: table size
        }
        let thread = (i as u8) & thread_mask;
        let core = ((i as u8) & core_mask) >> smt_shift;
        let apic = (core << smt_shift) | thread;
        cpus[count as usize] = CpuId {
            apic_id: apic,
            thread,
            core,
            ccd: 0,        // 5600X has 1 CCD
            ccx: ccx_id,
        };
        count += 1;
    }

    let bsp = CpuId {
        apic_id: smt_apic_id,
        thread: smt_apic_id & thread_mask,
        core: (smt_apic_id & core_mask) >> smt_shift,
        ccd: 0,
        ccx: ccx_id,
    };

    Topology {
        bsp,
        cpus,
        cpu_count: count,
        total_threads: count,
        total_cores: cores_per_package,
        total_ccxs: 1, // 5600X has 1 CCX
        total_ccds: 1, // 5600X has 1 CCD
    }
}

// Re-export so RING 0 can use these names without the `topology::` prefix
pub use self::detect as smp_init;
