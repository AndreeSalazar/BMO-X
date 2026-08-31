//! Per-CPU storage, addressed through GS after `swapgs`.
//!
//! [carril]  ROJO      el almacen por nucleo, direccionado por GS tras swapgs
//!
//! Ring 0 runs with `GS_BASE = &PerCpu` and `KERNEL_GS_BASE = 0` (the user
//! GS). Trap entries from Ring 3 execute `swapgs` first; entries that
//! interrupted Ring 0 skip it (they check the saved CS). After `init_bsp`,
//! any code can reach its per-CPU data as `[gs:OFF]` from asm, or through
//! the safe Rust accessors below (which read the same static slots on the
//! BSP -- APs get their own slots when SMP lands).
//!
//! Layout is part of the asm contract:
//! ```text
//!   [gs:0x00] syscall_stack_top   kernel stack for the SYSCALL entry
//!   [gs:0x08] user_rsp_scratch    user RSP saved by the SYSCALL entry
//!   [gs:0x10] trap_rsp            fxsave-base of the context on this CPU
//!   [gs:0x18] cpu_id / apic_id
//! ```

use core::sync::atomic::{AtomicUsize, Ordering};

pub const OFF_SYSCALL_STACK: u64 = 0x00;
pub const OFF_USER_RSP: u64 = 0x08;
pub const OFF_TRAP_RSP: u64 = 0x10;

pub const MAX_CPUS: usize = 16;
pub const SYSCALL_STACK_SIZE: usize = 32 * 1024;

const MSR_GS_BASE: u32 = 0xC000_0101;
const MSR_KERNEL_GS_BASE: u32 = 0xC000_0102;

#[repr(C, align(64))]
pub struct PerCpu {
    pub syscall_stack_top: u64,
    pub user_rsp_scratch: u64,
    pub trap_rsp: u64,
    pub cpu_id: u32,
    pub apic_id: u32,
}

const PER_CPU_ZERO: PerCpu = PerCpu {
    syscall_stack_top: 0,
    user_rsp_scratch: 0,
    trap_rsp: 0,
    cpu_id: 0,
    apic_id: 0,
};

static mut PER_CPUS: [PerCpu; MAX_CPUS] = [PER_CPU_ZERO; MAX_CPUS];

#[repr(align(64))]
struct SyscallStacks([[u8; SYSCALL_STACK_SIZE]; MAX_CPUS]);
static mut SYSCALL_STACKS: SyscallStacks = SyscallStacks([[0; SYSCALL_STACK_SIZE]; MAX_CPUS]);

static ONLINE: AtomicUsize = AtomicUsize::new(0);

fn wrmsr(msr: u32, value: u64) {
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
            options(nostack),
        );
    }
}

/// Set up the BSP's per-CPU record and point GS at it. Must run before the
/// SYSCALL MSRs are armed and before interrupts are enabled.
pub fn init_bsp() {
    let stack = unsafe { core::ptr::addr_of_mut!(SYSCALL_STACKS.0[0]) } as u64;
    let per_cpu = unsafe { &mut *core::ptr::addr_of_mut!(PER_CPUS[0]) };
    per_cpu.syscall_stack_top = stack + SYSCALL_STACK_SIZE as u64;
    per_cpu.user_rsp_scratch = 0;
    per_cpu.trap_rsp = 0;
    per_cpu.cpu_id = 0;
    per_cpu.apic_id = 0; // BSP APIC id is published by smp support later.
    let base = per_cpu as *mut PerCpu as u64;
    wrmsr(MSR_GS_BASE, base);
    // Ring 0 keeps GS_BASE on its PerCpu; the user GS is always 0, so the
    // first swapgs on a Ring 3 entry loads PER_CPU and stashes 0 away.
    wrmsr(MSR_KERNEL_GS_BASE, 0);
    ONLINE.store(1, Ordering::Release);
}

/// Read the fxsave-base stack pointer of the context currently on this CPU.
/// The trap epilogue returns to whatever this holds.
///
/// **gs-relative on purpose.** The trap entry stubs write this slot as
/// `mov gs:[0x10], rsp`; reading the `PER_CPUS` static instead silently
/// splits the brain the moment `GS_BASE != &PER_CPUS[0]` -- the stub's write
/// lands elsewhere, this read returns the never-written 0, and the epilogue
/// restores a context from address 0 (the observed #GP: backptr 0, pops to
/// rsp 0x78, iretq with cs=0). One access path = writer and reader agree by
/// construction, whatever GS_BASE holds.
pub fn trap_rsp() -> u64 {
    let v: u64;
    unsafe {
        core::arch::asm!("mov {}, gs:[0x10]", out(reg) v, options(nostack, readonly));
    }
    v
}

/// Called by the scheduler when it commits a context switch. gs-relative
/// for the same single-access-path reason as `trap_rsp`.
pub fn set_trap_rsp(rsp: u64) {
    unsafe {
        core::arch::asm!("mov gs:[0x10], {}", in(reg) rsp, options(nostack));
    }
}

/// Point the SYSCALL entry at the running task's kernel stack. The
/// scheduler updates this when it switches to a user task; syscalls only
/// ever arrive from the task currently on the CPU, so mid-dispatch updates
/// only affect the next entry. gs-relative: the SYSCALL entry reads this
/// slot as `mov rsp, gs:[0x00]`.
pub fn set_syscall_stack_top(top: u64) {
    unsafe {
        core::arch::asm!("mov gs:[0x00], {}", in(reg) top, options(nostack));
    }
}

/// Raw `(GS_BASE, KERNEL_GS_BASE, &PER_CPUS[0])` for diagnostics: the two
/// MSRs and the address they are SUPPOSED to hold. If GS_BASE differs from
/// the static's address, some path rewrote it (or an unbalanced swapgs).
pub fn gs_diag() -> (u64, u64, u64) {
    fn rdmsr(msr: u32) -> u64 {
        let (lo, hi): (u32, u32);
        unsafe {
            core::arch::asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi,
                options(nostack, readonly));
        }
        ((hi as u64) << 32) | lo as u64
    }
    let expected = unsafe { core::ptr::addr_of!(PER_CPUS[0]) } as u64;
    (rdmsr(MSR_GS_BASE), rdmsr(MSR_KERNEL_GS_BASE), expected)
}

pub fn cpu_count_online() -> usize {
    ONLINE.load(Ordering::Acquire)
}
