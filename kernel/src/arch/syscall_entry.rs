//! Syscall Entry Point — IA32_LSTAR / STAR / FMASK MSR setup.
//!
//! x86-64 `syscall` instruction:
//!   - Saves RIP in RCX, RFLAGS in R11
//!   - Loads CS from STAR[47:32], SS from STAR[47:32]+8
//!   - Loads RIP from IA32_LSTAR
//!   - Masks RFLAGS with IA32_FMASK
//!
//! Return to Ring 3 uses `iretq` (not `sysretq`) for safety.
//!
//! InterruptFrame layout MUST match the exact push/pop order and match
//! the iretq frame layout (rip, cs, rflags, rsp, ss) at the end so
//! that after popping GPRs we can directly `iretq`.
//!
//! BMO ABI syscall convention:
//!   RAX = syscall number
//!   RDI, RSI, RDX, R10, R8, R9 = arguments
//!   Return: RAX = result (negative = error)

use core::arch::{asm, naked_asm};

const IA32_STAR: u32  = 0xC000_0081;
const IA32_LSTAR: u32 = 0xC000_0082;
const IA32_FMASK: u32 = 0xC000_0084;
const IA32_EFER: u32  = 0xC000_0080;

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

pub fn init_syscall() {
    unsafe {
        let efer = rdmsr(IA32_EFER);
        wrmsr(IA32_EFER, efer | 1);

        let kernel_base: u64 = 0x08;
        let user_base: u64   = 0x10;
        let star = (user_base << 48) | (kernel_base << 32);
        wrmsr(IA32_STAR, star);

        wrmsr(IA32_LSTAR, syscall_entry_naked as *const () as u64);

        // Clear IF (bit 9) and DF (bit 10) on syscall entry
        wrmsr(IA32_FMASK, (1 << 9) | (1 << 10));
    }
}

/// Saved user context for iretq return.
///
/// Stack layout after building the frame + pushing GPRs (low to high):
///
///   GPR section (pushed by our code, restored by pop):
///     r15, r14, r13, r12, rbp, rbx, r9, r8, r10, rdx, rsi, rdi, rax
///
///   Iretq frame (must match what `iretq` expects, low to high):
///     rip, cs, rflags, user_rsp, ss
///
/// After popping all 13 GPRs, rsp points to `rip` → iretq pops
/// rip, cs, rflags, rsp, ss in order.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct InterruptFrame {
    // General-purpose registers (pushed in reverse: last push = lowest addr)
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
    // Iretq frame — matches CPU's iretq pop order (low to high)
    pub rip: u64,    // return instruction pointer
    pub cs: u64,     // code segment (0x23)
    pub rflags: u64, // CPU flags
    pub rsp: u64,    // user stack pointer
    pub ss: u64,     // stack segment (0x1B)
}

static mut SYSCALL_KERNEL_RSP: u64 = 0;

pub fn set_syscall_kernel_stack(rsp: u64) {
    unsafe { SYSCALL_KERNEL_RSP = rsp; }
}

pub fn ring3_alive() -> bool {
    unsafe { RING3_SYSCALL_SEEN }
}

/// Naked syscall entry point — called by hardware via IA32_LSTAR.
///
/// Correct flow:
///   1. Save user RSP, switch to kernel stack
///   2. Build iretq frame (rip, cs, rflags, rsp, ss) — pushed FIRST = lowest addr
///   3. Push GPRs on top — pushed AFTER = higher addr
///   4. Call Rust handler with pointer to the full InterruptFrame
///   5. On return: pop GPRs, then `iretq` directly (frame is already correct)
#[unsafe(naked)]
unsafe extern "C" fn syscall_entry_naked() {
    naked_asm!(
        // ── Step 1: Save user RSP, switch to kernel stack ──
        "mov r15, rsp",                         // r15 = user RSP
        "mov rsp, [rip + {kstack}]",            // switch to kernel stack

        // ── Step 2: Build iretq frame FIRST (lowest addresses) ──
        // Push order: ss, user_rsp, rflags, cs, rip
        // After these 5 pushes, [rsp+0]=rip [rsp+8]=cs [rsp+16]=rflags [rsp+24]=rsp [rsp+32]=ss
        "push qword ptr 0x1B",                  // ss (USER_DS | RPL=3)
        "push r15",                              // user RSP
        "push r11",                              // rflags (saved by CPU)
        "push qword ptr 0x23",                  // cs (USER_CS | RPL=3)
        "push rcx",                              // rip (saved by CPU)

        // ── Step 3: Push GPRs (higher addresses) ──
        // Push order: rax, rdi, rsi, rdx, r10, r8, r9, rbx, rbp, r12, r13, r14, r15
        // This matches the InterruptFrame struct layout (r15 at lowest addr).
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

        // ── Step 4: Call Rust handler ──
        "mov rdi, rsp",                         // arg 0: pointer to InterruptFrame
        "call {handler}",                       // call syscall_handler_rust

        // ── Step 5: Pop GPRs ──
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

        // ── Step 6: Return to Ring 3 ──
        // After popping GPRs, rsp points to the iretq frame:
        //   [rsp+0]  = rip
        //   [rsp+8]  = cs (0x23)
        //   [rsp+16] = rflags
        //   [rsp+24] = user rsp
        //   [rsp+32] = ss (0x1B)
        // This is exactly what iretq expects.
        "iretq",

        kstack = sym SYSCALL_KERNEL_RSP,
        handler = sym syscall_handler_rust,
    );
}

static mut RING3_SYSCALL_SEEN: bool = false;

/// Rust syscall handler — dispatches by syscall number.
///
/// `frame` points to the saved user context on the kernel stack.
/// The handler may modify frame fields — they will be restored on return.
#[unsafe(no_mangle)]
extern "C" fn syscall_handler_rust(frame: *mut InterruptFrame) {
    unsafe {
        if !RING3_SYSCALL_SEEN {
            RING3_SYSCALL_SEEN = true;
            crate::diag::info("ring3", "first syscall received; Ring 3 is alive");
        }

        let f = &mut *frame;
        let nr = f.rax;
        let a0 = f.rdi;
        let a1 = f.rsi;
        let a2 = f.rdx;
        let a3 = f.r10;
        let a4 = f.r8;
        let _a5 = f.r9;

        crate::diag::trace_u64("syscall", "dispatch nr", nr);

        let result = match nr {
            // ─── Procesos ─────────────────────────────────────────────
            0x00 => {
                crate::diag::trace("syscall", "ProcessExit");
                crate::sched::process::kill_current_process(0, a0, 0);
            }
            0x01 => u64::MAX,
            0x02 => u64::MAX,
            0x03 => {
                crate::sched::yield_now();
                0
            }
            0x04 => {
                // ThreadCreate(entry, stack, priority) → tid
                crate::diag::trace("syscall", "ThreadCreate");
                match crate::sched::thread::alloc_thread(
                    crate::sched::process::Pid(1),
                    crate::sched::Priority::Interactive,
                ) {
                    Some(thr) => {
                        thr.regs = crate::sched::thread::SavedRegs::new_user(a0, a1);
                        thr.state = crate::sched::thread::ThreadState::Ready;
                        thr.tid.0 as u64
                    }
                    None => u64::MAX,
                }
            }
            0x05 => {
                // ThreadExit(code) — kill current thread, reschedule
                crate::diag::trace("syscall", "ThreadExit");
                crate::sched::process::kill_current_process(0, a0, 0);
            }

            // ─── Memoria ──────────────────────────────────────────────
            0x10 => u64::MAX,
            0x11 => u64::MAX,
            0x12 => u64::MAX,

            // ─── VFS ──────────────────────────────────────────────────
            0x20 => crate::fs::ramdisk::open(a0, a1),
            0x21 => crate::fs::ramdisk::read(a0, a1, a2),
            0x22 => crate::fs::ramdisk::write(a0, a1, a2),
            0x23 => crate::fs::ramdisk::close(a0),
            0x24 => crate::fs::ramdisk::seek(a0, a1, a2),
            0x25 => crate::fs::ramdisk::size(a0),

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
            0x50 => crate::arch::cpu::rdtsc(),
            0x51 => {
                let target_ns = a0;
                let target_cycles = (target_ns as u128 * 37) / 10;
                let start = crate::arch::cpu::rdtsc();
                while (crate::arch::cpu::rdtsc() - start) < target_cycles as u64 {
                    core::hint::spin_loop();
                }
                0
            }

            // ─── Framebuffer ──────────────────────────────────────────
            0x60 => {
                let w = crate::boot_info::FB_WIDTH as u64;
                let h = crate::boot_info::FB_HEIGHT as u64;
                let s = crate::boot_info::FB_STRIDE as u64;
                w | (h << 32) | ((s & 0xFFFF) << 48)
            }
            0x61 => {
                crate::desktop::fb_fill(a0 as u32, a1 as u32, a2 as u32, a3 as u32, a4 as u32);
                0
            }
            0x62 => {
                if a3 > 0 && a3 < 256 {
                    let slice = core::slice::from_raw_parts(a2 as *const u8, a3 as usize);
                    crate::desktop::fb_text(a0 as u32, a1 as u32, slice, a4 as u32);
                }
                0
            }
            0x63 => 0,
            0x64 => {
                crate::desktop::fb_blit(a0 as u32, a1 as u32, a2 as u32, a3 as u32, a4);
                0
            }
            0x65 => {
                crate::desktop::render::render_frame();
                crate::diag::paint_overlay();
                crate::desktop::state::STATE.frame
            }

            // ─── Input ────────────────────────────────────────────────
            0x70 => crate::desktop::poll_key() as u64,
            0x71 => crate::desktop::poll_mouse(),

            // ─── Sonido ───────────────────────────────────────────────
            0x80 => {
                crate::desktop::beep(a0 as u32, a1 as u32);
                0
            }

            // ─── Security ─────────────────────────────────────────────
            0xA0 => {
                if a1 > 0 && a1 < 1024 * 1024 {
                    let data = core::slice::from_raw_parts(a0 as *const u8, a1 as usize);
                    crate::security::bytedefender::scanner::scan_memory(data).level as u64
                } else {
                    0
                }
            }
            0xA1 => {
                let state = crate::security::bytedefender::state();
                state.files_scanned | (state.threats_blocked << 32)
            }
            0xA2 => {
                let label = if a1 > 0 && a1 < 64 {
                    core::slice::from_raw_parts(a0 as *const u8, a1 as usize)
                } else {
                    b"manual"
                };
                crate::security::restaurer::create_snapshot(label, b"User snapshot")
            }
            0xA3 => {
                if crate::security::restaurer::rollback(a0) { 0 } else { 1 }
            }
            0xA4 => crate::security::restaurer::state().snapshot_count,

            // ─── Debug ────────────────────────────────────────────────
            0xF0 => {
                if a1 > 0 && a1 < 4096 {
                    let slice = core::slice::from_raw_parts(a0 as *const u8, a1 as usize);
                    if let Ok(s) = core::str::from_utf8(slice) {
                        crate::drivers::serial::serial_write(s);
                    }
                }
                0
            }

            _ => {
                crate::diag::warn_u64("syscall", "unknown syscall", nr);
                u64::MAX
            }
        };

        f.rax = result;
    }
}
