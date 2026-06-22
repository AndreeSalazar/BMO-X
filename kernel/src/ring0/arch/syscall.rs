//! Syscall Entry Point + Dispatcher.
//!
//! Combines the IA32_LSTAR/STAR/FMASK MSR setup, the naked `syscall`
//! entry, the Rust handler, the syscall number enum, and the futex
//! primitives into a single module. Syscall is fundamentally a part
//! of the x86-64 architecture, so it lives under `arch/`.
//!
//! ## ABI
//!
//! x86-64 `syscall` instruction saves RIP in RCX, RFLAGS in R11, and
//! jumps to `IA32_LSTAR`. Arguments are in RDI, RSI, RDX, R10, R8, R9.
//! Return value goes in RAX. We return to Ring 3 via `iretq` (not
//! `sysretq`) for safety.
//!
//! ## MSR setup
//!
//! - **IA32_EFER** bit 0: SCE (System Call Extensions)
//! - **IA32_STAR[47:32]**: kernel CS selector (0x08)
//! - **IA32_STAR[63:48]**: kernel DS selector (0x10, loaded as SS)
//! - **IA32_LSTAR**: entry point of `syscall_entry_naked`
//! - **IA32_FMASK**: clear IF (bit 9) and DF (bit 10) on entry

#![allow(dead_code, static_mut_refs)]

use core::arch::{asm, naked_asm};
use alloc::collections::VecDeque;

// ── MSR addresses (cached for clarity) ───────────────────────────────────────

const IA32_STAR: u32             = 0xC000_0081;
const IA32_LSTAR: u32            = 0xC000_0082;
const IA32_FMASK: u32            = 0xC000_0084;
const IA32_EFER: u32             = 0xC000_0080;
const IA32_GS_BASE: u32          = 0xC000_0101;
const IA32_KERNEL_GS_BASE: u32   = 0xC000_0102;

/// Segment selectors matching `crate::arch::gdt`.
const KERNEL_CS_SELECTOR: u64 = 0x08;
const KERNEL_DS_SELECTOR: u64 = 0x10;

// ── MSR read/write helpers ───────────────────────────────────────────────────

#[inline]
unsafe fn wrmsr(msr: u32, val: u64) {
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    asm!("wrmsr", in("ecx") msr, in("eax") lo, in("edx") hi, options(nostack));
}

#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nostack));
    ((hi as u64) << 32) | (lo as u64)
}

/// Read the current IA32_GS_BASE.
#[inline(always)]
pub unsafe fn read_gs_base() -> u64 { rdmsr(IA32_GS_BASE) }

/// Set the kernel's IA32_KERNEL_GS_BASE.
#[inline(always)]
pub unsafe fn write_kernel_gs_base(v: u64) { wrmsr(IA32_KERNEL_GS_BASE, v); }

/// Set the user's IA32_GS_BASE.
#[inline(always)]
pub unsafe fn write_user_gs_base(v: u64) { wrmsr(IA32_GS_BASE, v); }

/// Switch from user GS to kernel GS.
#[inline(always)]
pub unsafe fn swapgs() {
    asm!("swapgs", options(nostack, preserves_flags));
}

// ── Trait: implementable by any syscall handler ─────────────────────────────

/// Trait for dispatching a syscall by number.
///
/// Implementors receive the syscall number and 6 argument registers
/// (in BMO ABI order: rdi, rsi, rdx, r10, r8, r9). They return a
/// `u64` (negative = error per POSIX, ≥0 = success).
pub trait SyscallHandler {
    fn handle(&self, nr: u16, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> u64;
}

// ── Init ─────────────────────────────────────────────────────────────────────

/// Per-CPU kernel stack pointer used by the syscall entry.
static mut SYSCALL_KERNEL_RSP: u64 = 0;

pub fn init_syscall() {
    unsafe {
        // Enable SCE in EFER.
        let efer = rdmsr(IA32_EFER);
        wrmsr(IA32_EFER, efer | 1);

        // STAR: kernel CS=0x08 (bits 47:32), kernel DS=0x10 (bits 63:48).
        let star = (KERNEL_DS_SELECTOR << 48) | (KERNEL_CS_SELECTOR << 32);
        wrmsr(IA32_STAR, star);

        // LSTAR: entry point.
        wrmsr(IA32_LSTAR, syscall_entry_naked as *const () as u64);

        // FMASK: clear IF (bit 9) and DF (bit 10) on entry.
        wrmsr(IA32_FMASK, (1 << 9) | (1 << 10));

        // GS bases (we use 0; no per-CPU data in v1.7.5).
        wrmsr(IA32_KERNEL_GS_BASE, 0);
        wrmsr(IA32_GS_BASE, 0);

        // Initialize the syscall kernel stack to the global KERNEL_STACK.
        let stack_top = crate::arch::gdt::kernel_stack_top();
        SYSCALL_KERNEL_RSP = stack_top;
    }

    // v1.8.8: re-initialize ALL common MSRs (EFER, STAR, LSTAR, FMASK, PAT,
    // TSC_AUX, GS bases) using the real syscall entry point. This overrides
    // the placeholder that `init_fastos_cpu()` wrote in coordinator::main.
    let real_entry = syscall_entry_naked as *const () as u64;
    crate::vendor::amd::cpu::zen3::init_msrs(real_entry);
}

/// Set the kernel stack pointer used by the syscall entry.
pub fn set_syscall_kernel_stack(rsp: u64) {
    unsafe { SYSCALL_KERNEL_RSP = rsp; }
}

// ── Saved user ctx ───────────────────────────────────────────────────────

/// Saved user ctx for iretq return.
///
/// Stack layout after building the frame + pushing GPRs (low to high):
///   GPR section (pushed by our code, restored by pop):
///     r15, r14, r13, r12, rbp, rbx, r9, r8, r10, rdx, rsi, rdi, rax
///   Iretq frame (must match what `iretq` expects, low to high):
///     rip, cs, rflags, user_rsp, ss
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct InterruptFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub r9: u64,
    pub r8: u64,
    pub r10: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rax: u64,    // syscall number
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

// ── Naked entry point ────────────────────────────────────────────────────────

#[unsafe(naked)]
unsafe extern "C" fn syscall_entry_naked() {
    naked_asm!(
        "swapgs",
        "mov r15, rsp",
        "mov rsp, [rip + {kstack}]",
        "push qword ptr 0x1B",                  // ss
        "push r15",                              // user RSP
        "push r11",                              // rflags
        "push qword ptr 0x23",                  // cs
        "push rcx",                              // rip
        "push rax",
        "push rdi",
        "push rsi",
        "push rdx",
        "push r10",
        "push r8",
        "push r9",
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov rdi, rsp",
        "call {handler}",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",
        "pop r9",
        "pop r8",
        "pop r10",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop rax",
        "swapgs",
        "iretq",
        kstack = sym SYSCALL_KERNEL_RSP,
        handler = sym syscall_handler_rust,
    );
}

// ── Rust handler ─────────────────────────────────────────────────────────────

static mut RING3_SYSCALL_SEEN: bool = false;

pub fn ring3_alive() -> bool {
    unsafe { RING3_SYSCALL_SEEN }
}

#[unsafe(no_mangle)]
extern "C" fn syscall_handler_rust(frame: *mut InterruptFrame) {
    unsafe {
        if !RING3_SYSCALL_SEEN {
            RING3_SYSCALL_SEEN = true;
            crate::bmo_core::diag::info("ring3", "first syscall received; Ring 3 is alive");
        }

        let f = &mut *frame;
        let nr = f.rax;
        let a0 = f.rdi;
        let a1 = f.rsi;
        let a2 = f.rdx;
        let a3 = f.r10;
        let a4 = f.r8;
        let a5 = f.r9;

        crate::bmo_core::diag::trace_u64("syscall", "dispatch nr", nr);

        let result = match nr {
            // ─── BMO API v2 (0x100..=0x1FF) ──────────────────────────
            n if (0x100..=0x1FF).contains(&(n as u16)) => {
                crate::bmo_core::diag::trace("syscall", "BMO API v2 dispatch");
                crate::bmo_core::bmo_api::dispatch_syscall(n as u16, a0, a1, a2, a3, a4, a5)
            }

            // ─── Procesos ─────────────────────────────────────────────
            0x00 => {
                crate::bmo_core::diag::trace("syscall", "ProcessExit");
                crate::proc::process::kill_current_process(0, a0, 0);
            }
            0x01 => u64::MAX,
            0x02 => u64::MAX,
            0x03 => {
                crate::proc::yield_now();
                0
            }
            0x04 => {
                crate::bmo_core::diag::trace("syscall", "ThreadCreate");
                match crate::proc::task::alloc(
                    crate::proc::process::Pid(1),
                    crate::proc::Priority::Interactive,
                ) {
                    Some(thr) => {
                        thr.regs = crate::proc::task::SavedRegs::new_user(a0, a1);
                        thr.state = crate::proc::task::State::Ready;
                        thr.tid.0 as u64
                    }
                    None => u64::MAX,
                }
            }
            0x05 => {
                crate::bmo_core::diag::trace("syscall", "ThreadExit");
                crate::proc::process::kill_current_process(0, a0, 0);
            }

            // ─── Memoria ──────────────────────────────────────────────
            0x10 => u64::MAX,
            0x11 => u64::MAX,
            0x12 => u64::MAX,

            // ─── VFS ──────────────────────────────────────────────────
            0x20 => crate::bmo_core::fs::ramdisk::open(a0, a1),
            0x21 => crate::bmo_core::fs::ramdisk::read(a0, a1, a2),
            0x22 => crate::bmo_core::fs::ramdisk::write(a0, a1, a2),
            0x23 => crate::bmo_core::fs::ramdisk::close(a0),
            0x24 => crate::bmo_core::fs::ramdisk::seek(a0, a1, a2),
            0x25 => crate::bmo_core::fs::ramdisk::size(a0),

            // ─── IPC ──────────────────────────────────────────────────
            0x30 => u64::MAX,
            0x31 => u64::MAX,
            0x32 => u64::MAX,

            // ─── BareX bridges ────────────────────────────────────────
            0x40 => u64::MAX,
            0x41 => u64::MAX,
            0x42 => u64::MAX,
            0x43 => u64::MAX,

            // ─── Tiempo ───────────────────────────────────────────────
            0x50 => crate::cpu::rdtsc(),
            0x51 => {
                let target_ns = a0;
                let target_cycles = (target_ns as u128 * 37) / 10;
                let start = crate::cpu::rdtsc();
                while (crate::cpu::rdtsc() - start) < target_cycles as u64 {
                    core::hint::spin_loop();
                }
                0
            }

            // ─── Framebuffer ──────────────────────────────────────────
            0x60 => {
                let w = crate::boot::info::FB_WIDTH as u64;
                let h = crate::boot::info::FB_HEIGHT as u64;
                let s = crate::boot::info::FB_STRIDE as u64;
                w | (h << 32) | ((s & 0xFFFF) << 48)
            }
            0x61 => {
                crate::bmo_core::desktop::fb_fill(a0 as u32, a1 as u32, a2 as u32, a3 as u32, a4 as u32);
                0
            }
            0x62 => {
                if a3 > 0 && a3 < 256 {
                    let slice = core::slice::from_raw_parts(a2 as *const u8, a3 as usize);
                    crate::bmo_core::desktop::fb_text(a0 as u32, a1 as u32, slice, a4 as u32);
                }
                0
            }
            0x63 => 0,
            0x64 => {
                crate::bmo_core::desktop::fb_blit(a0 as u32, a1 as u32, a2 as u32, a3 as u32, a4);
                0
            }
            0x65 => {
                crate::bmo_core::desktop::render::render_frame();
                crate::bmo_core::diag::paint_overlay();
                crate::bmo_core::desktop::state::STATE.frame
            }

            // ─── Input ────────────────────────────────────────────────
            0x70 => crate::bmo_core::desktop::poll_key() as u64,
            0x71 => crate::bmo_core::desktop::poll_mouse(),

            // ─── Sonido ───────────────────────────────────────────────
            0x80 => {
                crate::bmo_core::desktop::beep(a0 as u32, a1 as u32);
                0
            }

            // ─── Debug ────────────────────────────────────────────────
            0xF0 => {
                if a1 > 0 && a1 < 4096 {
                    let slice = core::slice::from_raw_parts(a0 as *const u8, a1 as usize);
                    if let Ok(s) = core::str::from_utf8(slice) {
                        crate::dev::console::serial_write(s);
                    }
                }
                0
            }

            _ => {
                crate::bmo_core::diag::warn_u64("syscall", "unknown syscall", nr);
                u64::MAX
            }
        };

        f.rax = result;
    }
}

// ── Syscall numbers (legacy 0x00..0xFF) ─────────────────────────────────────

/// Syscall numbers for the legacy 0x00..0xFF range.
///
/// Note: the BMO API v2 (0x100..0x1FF) is dispatched directly to
/// `bmo_core::bmo_api::dispatch_syscall` and is not enumerated here.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syscall {
    ProcessExit         = 0x00,
    ThreadCreate        = 0x01,
    ThreadExit          = 0x02,
    ThreadYield         = 0x03,
    FutexWait           = 0x04,
    FutexWake           = 0x05,
    Mmap                = 0x10,
    Munmap              = 0x11,
    Mprotect            = 0x12,
    FileOpen            = 0x20,
    FileRead            = 0x21,
    FileWrite           = 0x22,
    FileClose           = 0x23,
    FileSeek            = 0x24,
    FileStat            = 0x25,
    PortCreate          = 0x30,
    PortSend            = 0x31,
    PortRecv            = 0x32,
    BarexGfxSubmit      = 0x40,
    BarexAudioSubmit    = 0x41,
    BarexInputPoll      = 0x42,
    BarexNetSubmit      = 0x43,
    ClockGetTime        = 0x50,
    NanoSleep           = 0x51,
    DebugPrint          = 0xF0,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SyscallFrame {
    pub rax: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub r10: u64,
    pub r8:  u64,
    pub r9:  u64,
}

// ── Futex (single-core) ──────────────────────────────────────────────────────

struct FutexWaiter {
    addr: *const u32,
    woken: bool,
}

static mut FUTEX_QUEUE: VecDeque<FutexWaiter> = VecDeque::new();

/// Kernel-side futex_wait: if `*addr == expected`, suspend current thread.
pub fn futex_wait(addr: *const u32, expected: u32, _timeout_ns: u64) -> bool {
    unsafe {
        if core::ptr::read_volatile(addr) != expected {
            return false;
        }
        FUTEX_QUEUE.push_back(FutexWaiter { addr, woken: false });
        crate::proc::yield_now();
        true
    }
}

/// Wake up to `count` threads waiting on `addr`.
pub fn futex_wake(addr: *const u32, count: u32) -> u32 {
    unsafe {
        let mut woken = 0u32;
        let mut i = 0;
        while i < FUTEX_QUEUE.len() && woken < count {
            if FUTEX_QUEUE[i].addr == addr && !FUTEX_QUEUE[i].woken {
                FUTEX_QUEUE[i].woken = true;
                woken += 1;
            }
            i += 1;
        }
        FUTEX_QUEUE.retain(|w| !w.woken);
        woken
    }
}

// NOTA: `init()` quedó como wrapper trivial de `init_syscall()`. v1.8.7:
// los únicos call sites usan `init_syscall()` directamente, así que este
// wrapper se elimina. Si en el futuro se quiere re-unificar, exponer un
// solo nombre y llamarlo desde `p0_arch::run`.
