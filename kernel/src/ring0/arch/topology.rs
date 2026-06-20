//! CPU topology and per-CPU data structures.
//!
//! On the Ryzen 5 5600X (Zen 3 / Vermeer), the APIC ID encodes
//! thread/core/CCD position in a known way:
//!
//! ```text
//!   bits[1:0]  thread within core       (0 or 1)
//!   bits[3:2]  core within CCD          (0..5 on 5600X)
//!   bits[5:4]  CCD ID                   (0 on 5600X, single CCD)
//! ```
//!
//! # Per-CPU data
//!
//! Each thread has its own data area, accessed via `IA32_GS_BASE` and
//! `IA32_KERNEL_GS_BASE`. On syscall entry, `swapgs` exchanges the two;
//! the kernel then uses `gs:0` to find its per-CPU data.

#![allow(dead_code)]
#![allow(static_mut_refs)]

use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::cpu::msr;

/// Number of APIC IDs we support. 64 is enough for current CPUs.
pub const MAX_THREADS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuId {
    pub apic_id: u32,
    pub thread: u8,
    pub core: u8,
    pub ccd: u8,
}

impl CpuId {
    pub const fn from_apic(apic_id: u32) -> Self {
        Self {
            apic_id,
            thread: (apic_id & 0x3) as u8,
            core: ((apic_id >> 2) & 0xF) as u8,
            ccd: ((apic_id >> 4) & 0xF) as u8,
        }
    }

    pub fn is_smt_sibling_of(&self, other: &CpuId) -> bool {
        self.ccd == other.ccd && self.core == other.core && self.thread != other.thread
    }

    pub fn same_ccd_as(&self, other: &CpuId) -> bool {
        self.ccd == other.ccd
    }
}

/// Return the BSP's APIC ID by reading the local APIC register.
pub fn bsp_apic_id() -> u32 {
    const APIC_BASE: u64 = 0xFEE0_0000;
    const APIC_ID_REG: u32 = 0x0020;
    unsafe {
        let p = (APIC_BASE + APIC_ID_REG as u64) as *const u32;
        (p.read_volatile() >> 24) & 0xFF
    }
}

/// Static description of the CPU topology, detected at boot.
#[derive(Debug, Clone)]
pub struct Topology {
    pub total_threads: u32,
    pub core_count: u32,
    pub ccd_count: u32,
    pub threads_per_core: u8,
    pub cores_per_ccd: u8,
    pub bsp: CpuId,
    pub apic_ids: [u32; MAX_THREADS],
}

impl Topology {
    pub const fn empty() -> Self {
        Self {
            total_threads: 1,
            core_count: 1,
            ccd_count: 1,
            threads_per_core: 1,
            cores_per_ccd: 1,
            bsp: CpuId { apic_id: 0, thread: 0, core: 0, ccd: 0 },
            apic_ids: [u32::MAX; MAX_THREADS],
        }
    }

    pub fn index_of(&self, apic_id: u32) -> Option<usize> {
        self.apic_ids[..self.total_threads as usize]
            .iter()
            .position(|&a| a == apic_id)
    }
}

/// Detect the topology. On a 5600X: 12 threads, 6 cores, 1 CCD, 2 threads/core.
pub fn detect() -> Topology {
    let mut topo = Topology::empty();

    let (max_leaf, _, _, _) = cpuid(0, 0);
    let mut total_threads = 1u32;
    if max_leaf >= 0x0B {
        let (_, ebx, _, _) = cpuid(0x0B, 0);
        total_threads = (ebx & 0xFFFF).max(1);
    }
    topo.total_threads = total_threads;

    let (max_ext, _, _, _) = cpuid(0x8000_0000, 0);
    if max_ext >= 0x8000_001E {
        let (_, ebx, _, _) = cpuid(0x8000_001E, 0);
        let threads_per_cu = (ebx & 0xFF) as u8;
        topo.threads_per_core = if threads_per_cu == 0 { 1 } else { threads_per_cu };
        topo.core_count = (total_threads + topo.threads_per_core as u32 - 1)
            / topo.threads_per_core as u32;
    }
    topo.ccd_count = 1;
    topo.cores_per_ccd = topo.core_count as u8;

    let bsp_id = bsp_apic_id();
    topo.bsp = CpuId::from_apic(bsp_id);

    for i in 0..total_threads.min(MAX_THREADS as u32) {
        topo.apic_ids[i as usize] = i;
    }

    topo
}

// ── Per-CPU data ────────────────────────────────────────────────────────────

/// Per-CPU data area. Each thread has its own `PerCpu`, indexed by APIC ID.
#[repr(C)]
pub struct PerCpu {
    pub magic: u32,
    pub apic_id: u32,
    pub kernel_stack_top: AtomicU64,
    pub online: AtomicBool,
    pub running: AtomicBool,
    pub idle_task: AtomicU64,
}

const PERCPU_MAGIC: u32 = 0xBEEF_C0DE;
const PERCPU_SIZE: usize = 64; // 64 bytes per slot, cache-line aligned

/// Global per-CPU array. Allocated at boot.
static mut PERCPU_TABLE: [PerCpu; MAX_THREADS] = [const {
    PerCpu {
        magic: 0,
        apic_id: u32::MAX,
        kernel_stack_top: AtomicU64::new(0),
        online: AtomicBool::new(false),
        running: AtomicBool::new(false),
        idle_task: AtomicU64::new(0),
    }
}; MAX_THREADS];

/// Initialize the per-CPU table. Call once during boot.
pub fn init(topology: &Topology) {
    unsafe {
        for (i, slot) in PERCPU_TABLE.iter_mut().enumerate() {
            slot.magic = 0;
            slot.apic_id = if i < topology.total_threads as usize {
                topology.apic_ids[i]
            } else {
                u32::MAX
            };
            slot.kernel_stack_top.store(0, Ordering::SeqCst);
            slot.online.store(false, Ordering::SeqCst);
            slot.running.store(false, Ordering::SeqCst);
            slot.idle_task.store(0, Ordering::SeqCst);
        }
    }
    let _ = PERCPU_SIZE; // silence unused
}

/// Initialize one per-CPU entry.
pub fn init_for_apic(apic_id: u32, kernel_stack_top: u64) -> Option<usize> {
    unsafe {
        let idx = PERCPU_TABLE.iter().position(|p| p.apic_id == apic_id)?;
        let slot = &mut PERCPU_TABLE[idx];
        slot.magic = PERCPU_MAGIC;
        slot.kernel_stack_top.store(kernel_stack_top, Ordering::SeqCst);
        slot.online.store(true, Ordering::SeqCst);
        slot.running.store(true, Ordering::SeqCst);
        slot.idle_task.store(0, Ordering::SeqCst);
        Some(idx)
    }
}

/// Get a reference to the current thread's per-CPU data. Requires
/// `swapgs` to have been called so GS points to the per-CPU area.
pub fn current() -> Option<&'static PerCpu> {
    let p: u64;
    unsafe { asm!("mov {}, gs:0", out(reg) p, options(nomem, preserves_flags)) };
    if p == 0 { return None; }
    let percpu = unsafe { &*(p as *const PerCpu) };
    if percpu.magic == PERCPU_MAGIC {
        Some(percpu)
    } else {
        None
    }
}

/// Set IA32_GS_BASE to point at the per-CPU area for `apic_id`.
pub fn set_gs_base_for_apic(apic_id: u32) {
    unsafe {
        let idx = PERCPU_TABLE
            .iter()
            .position(|p| p.apic_id == apic_id)
            .expect("set_gs_base_for_apic: unknown APIC ID");
        let addr = &PERCPU_TABLE[idx] as *const PerCpu as u64;
        msr::wrmsr(msr::IA32_GS_BASE, addr);
        msr::wrmsr(msr::IA32_KERNEL_GS_BASE, addr);
    }
}

/// Mark a thread as offline.
pub fn mark_offline(apic_id: u32) {
    unsafe {
        if let Some(slot) = PERCPU_TABLE.iter_mut().find(|p| p.apic_id == apic_id) {
            slot.online.store(false, Ordering::SeqCst);
            slot.running.store(false, Ordering::SeqCst);
        }
    }
}

/// Number of currently online threads.
pub fn online_count() -> u32 {
    let mut count = 0;
    unsafe {
        for slot in PERCPU_TABLE.iter() {
            if slot.online.load(Ordering::SeqCst) {
                count += 1;
            }
        }
    }
    count
}

#[inline]
fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (eax, ebx, ecx, edx);
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") leaf => eax,
            inout("ecx") sub => ecx,
            ebx_out = out(reg) ebx,
            out("edx") edx,
        );
    }
    (eax, ebx, ecx, edx)
}
