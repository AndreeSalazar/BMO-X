//! Per-CPU data — accessed via GS-base with `swapgs`.
//!
//! Each core has its own `PerCpu` struct. The BSP sets up its own GS-base
//! during MSR init; AP cores set theirs during AP startup.
//!
//! Usage from Ring 0:
//!   let pc = percpu::current();
//!   pc.core_id
//!   pc.kernel_stack_top
//!
//! This module is arch-agnostic: GS-base swapgs works identically on
//! Intel and AMD x86-64.

use core::arch::asm;

/// Maximum number of CPUs supported.
pub const MAX_CPUS: usize = 128;

/// Per-CPU data structure. One instance per logical processor.
/// Accessed via `gs:[0]` after `swapgs`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PerCpu {
    /// Logical core ID (0-based index into the CPU table).
    pub core_id: u32,
    /// APIC ID (hardware ID, used for IPI targeting).
    pub apic_id: u32,
    /// Top of the kernel stack for this core (used on interrupt entry).
    pub kernel_stack_top: u64,
    /// Stack pointer saved during context switch (scheduler uses this).
    pub saved_rsp: u64,
    /// Pointer to the current Task (index into global task table).
    pub current_task: u32,
    /// Is this core online and running?
    pub online: bool,
    /// Is this core currently idle (executing hlt loop)?
    pub idle: bool,
    /// Preemption disabled depth (nested cli/sti counter).
    pub preempt_count: u32,
    /// Interrupt disabled depth (nested cli counter).
    pub irq_disable_count: u32,
}

impl PerCpu {
    pub const fn empty() -> Self {
        Self {
            core_id: u32::MAX,
            apic_id: u32::MAX,
            kernel_stack_top: 0,
            saved_rsp: 0,
            current_task: u32::MAX,
            online: false,
            idle: false,
            preempt_count: 0,
            irq_disable_count: 0,
        }
    }
}

/// Static array of per-CPU data. Indexed by core_id.
static mut PERCPU_DATA: [PerCpu; MAX_CPUS] = {
    const EMPTY: PerCpu = PerCpu::empty();
    [EMPTY; MAX_CPUS]
};

/// Number of online CPUs.
static mut ONLINE_COUNT: u32 = 0;

/// Initialize the BSP's per-CPU data. Must be called once during boot.
pub fn init_bsp(apic_id: u32, kernel_stack_top: u64) {
    unsafe {
        let pc = &mut PERCPU_DATA[0];
        pc.core_id = 0;
        pc.apic_id = apic_id;
        pc.kernel_stack_top = kernel_stack_top;
        pc.online = true;
        pc.idle = false;

        // Set GS-base to point to this per-CPU struct.
        // After this, `swapgs; mov gs:[offset]` accesses this data.
        set_gs_base(pc as *const PerCpu as u64);
        ONLINE_COUNT = 1;
    }
}

/// Register a newly started AP core. Returns its core_id.
pub fn register_ap(apic_id: u32, kernel_stack_top: u64) -> u32 {
    unsafe {
        let id = ONLINE_COUNT;
        if id as usize >= MAX_CPUS {
            return u32::MAX;
        }
        let pc = &mut PERCPU_DATA[id as usize];
        pc.core_id = id;
        pc.apic_id = apic_id;
        pc.kernel_stack_top = kernel_stack_top;
        pc.online = true;
        pc.idle = false;
        ONLINE_COUNT += 1;
        id
    }
}

/// Get a reference to the current CPU's per-Cpu data.
/// Uses `gs:[0]` via inline assembly (swapgs not needed in Ring 0
/// when KERNEL_GS_BASE is set to the per-CPU struct).
#[inline]
pub fn current() -> &'static PerCpu {
    unsafe {
        let ptr: *const PerCpu;
        asm!("mov {}, gs:[0]", out(reg) ptr, options(nostack, readonly));
        &*ptr
    }
}

/// Get a mutable reference to the current CPU's per-Cpu data.
#[inline]
pub unsafe fn current_mut() -> &'static mut PerCpu {
    let ptr: *mut PerCpu;
    asm!("mov {}, gs:[0]", out(reg) ptr, options(nostack));
    &mut *ptr
}

/// Get a reference to a specific CPU's per-Cpu data by core_id.
pub fn get(core_id: u32) -> Option<&'static PerCpu> {
    unsafe {
        if core_id as usize >= MAX_CPUS { return None; }
        let pc = &PERCPU_DATA[core_id as usize];
        if pc.online { Some(pc) } else { None }
    }
}

/// Get the number of online CPUs.
pub fn online_count() -> u32 {
    unsafe { ONLINE_COUNT }
}

/// Find a PerCpu by APIC ID.
pub fn find_by_apic(apic_id: u32) -> Option<&'static PerCpu> {
    unsafe {
        for i in 0..ONLINE_COUNT {
            let pc = &PERCPU_DATA[i as usize];
            if pc.apic_id == apic_id {
                return Some(pc);
            }
        }
        None
    }
}

// ── GS-base MSR access ─────────────────────────────────────────────────

const MSR_IA32_GS_BASE: u32 = 0xC0000101;
const MSR_IA32_KERNEL_GS_BASE: u32 = 0xC0000102;

/// Set the GS-base for Ring 0 (used by `gs:[0]` in kernel mode).
#[inline]
pub unsafe fn set_gs_base(addr: u64) {
    asm!("wrmsr", in("ecx") MSR_IA32_GS_BASE,
         in("eax") (addr as u32), in("edx") ((addr >> 32) as u32),
         options(nostack));
}

/// Set the kernel GS-base (swapped by `swapgs`).
#[inline]
pub unsafe fn set_kernel_gs_base(addr: u64) {
    asm!("wrmsr", in("ecx") MSR_IA32_KERNEL_GS_BASE,
         in("eax") (addr as u32), in("edx") ((addr >> 32) as u32),
         options(nostack));
}

/// Read the current GS-base.
#[inline]
pub fn read_gs_base() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!("rdmsr", in("ecx") MSR_IA32_GS_BASE,
             out("eax") lo, out("edx") hi, options(nostack));
    }
    ((hi as u64) << 32) | (lo as u64)
}
