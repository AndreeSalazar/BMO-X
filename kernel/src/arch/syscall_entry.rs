//! Syscall Entry Point — IA32_LSTAR / STAR / FMASK MSR setup.
//!
//! x86-64 `syscall` instruction:
//!   - Saves RIP in RCX, RFLAGS in R11
//!   - Loads CS from STAR[47:32], SS from STAR[47:32]+8
//!   - Loads RIP from IA32_LSTAR
//!   - Masks RFLAGS with IA32_FMASK
//!
//! Return to Ring 3 uses `iretq` (not `sysretq`) for safety:
//!   - Pops RIP, CS, RFLAGS, RSP, SS from kernel stack
//!   - Handles exceptions properly (sysretq cannot return to faulting state)
//!
//! BMO ABI syscall convention:
//!   RAX = syscall number
//!   RDI, RSI, RDX, R10, R8, R9 = arguments
//!   Return: RAX = result (negative = error)

use core::arch::{asm, naked_asm};

// MSR addresses
const IA32_STAR: u32  = 0xC000_0081;
const IA32_LSTAR: u32 = 0xC000_0082;
const IA32_FMASK: u32 = 0xC000_0084;
const IA32_EFER: u32  = 0xC000_0080;

/// Write a 64-bit value to an MSR.
#[inline]
unsafe fn wrmsr(msr: u32, val: u64) {
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    asm!("wrmsr", in("ecx") msr, in("eax") lo, in("edx") hi, options(nostack));
}

/// Read a 64-bit value from an MSR.
#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nostack));
    ((hi as u64) << 32) | (lo as u64)
}

/// Initialize syscall/sysret MSRs.
///
/// Must be called AFTER `init_gdt()` since it references segment selectors.
pub fn init_syscall() {
    unsafe {
        // Enable SCE (System Call Extension) in IA32_EFER
        let efer = rdmsr(IA32_EFER);
        wrmsr(IA32_EFER, efer | 1); // bit 0 = SCE

        // IA32_STAR:
        //   bits 47:32 = kernel CS (for syscall) — CS = STAR[47:32], SS = STAR[47:32]+8
        //   bits 63:48 = user CS base (for sysret) — CS = STAR[63:48]+16, SS = STAR[63:48]+8
        //
        // With our GDT layout:
        //   Kernel CS = 0x08, Kernel SS = 0x10
        //   User CS   = 0x20 (0x18 + 16 = 0x28... no)
        //
        // sysret loads CS = STAR[63:48]+16, SS = STAR[63:48]+8
        // We want CS = 0x23 (User Code, RPL=3), SS = 0x1B (User Data, RPL=3)
        // So STAR[63:48] = 0x18 → CS = 0x18+16 = 0x28... but User Code is at 0x20!
        //
        // Actually sysret adds 16 to get CS for 64-bit mode.
        // STAR[63:48] should be 0x10:
        //   SS = 0x10 + 8 = 0x18 (User Data selector) → plus RPL=3 from CPU = 0x1B ✓
        //   CS = 0x10 + 16 = 0x20 (User Code selector) → plus RPL=3 from CPU = 0x23 ✓
        //
        // For syscall: STAR[47:32] = 0x08:
        //   CS = 0x08 (Kernel Code) ✓
        //   SS = 0x08 + 8 = 0x10 (Kernel Data) ✓
        let kernel_base: u64 = 0x08; // STAR[47:32]
        let user_base: u64   = 0x10; // STAR[63:48]
        let star = (user_base << 48) | (kernel_base << 32);
        wrmsr(IA32_STAR, star);

        // IA32_LSTAR = address of syscall entry point
        wrmsr(IA32_LSTAR, syscall_entry_naked as *const () as u64);

        // IA32_FMASK = RFLAGS bits to clear on syscall
        // Clear IF (bit 9) to disable interrupts during syscall entry,
        // and DF (bit 10) for consistent string ops direction.
        wrmsr(IA32_FMASK, (1 << 9) | (1 << 10));
    }
}

/// Saved user context for iretq return.
/// Layout must match the push order in syscall_entry_naked.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct InterruptFrame {
    pub rax: u64,    // syscall number (saved for dispatch)
    pub rdi: u64,    // arg 0
    pub rsi: u64,    // arg 1
    pub rdx: u64,    // arg 2
    pub r10: u64,    // arg 3
    pub r8: u64,     // arg 4
    pub r9: u64,     // arg 5
    pub rbx: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,    // user return address
    pub cs: u64,     // user code segment
    pub rflags: u64, // user flags
    pub rsp: u64,    // user stack pointer
    pub ss: u64,     // user data segment
}

/// Kernel stack for syscall entry. Updated by set_syscall_kernel_stack().
#[unsafe(no_mangle)]
static mut SYSCALL_KERNEL_RSP: u64 = 0;

/// Set the kernel stack pointer used by the syscall entry trampoline.
pub fn set_syscall_kernel_stack(rsp: u64) {
    unsafe { SYSCALL_KERNEL_RSP = rsp; }
}

/// Naked syscall entry point — called by hardware via IA32_LSTAR.
///
/// On entry from `syscall`:
///   RCX = saved RIP (user return address)
///   R11 = saved RFLAGS
///   RAX = syscall number
///   RDI, RSI, RDX, R10, R8, R9 = arguments
///   RSP = still user RSP! We must switch to kernel stack.
///
/// Stack layout after saving (for iretq return):
///   [rsp+0]  ss
///   [rsp+8]  rsp (user)
///   [rsp+16] rflags
///   [rsp+24] cs
///   [rsp+32] rip
///   [rsp+40] r9
///   [rsp+48] r8
///   [rsp+56] r10
///   [rsp+64] rdx
///   [rsp+72] rsi
///   [rsp+80] rdi
///   [rsp+88] rax
///   [rsp+96] rbx
///   [rsp+104] rbp
///   [rsp+112] r12
///   [rsp+120] r13
///   [rsp+128] r14
///   [rsp+136] r15
#[unsafe(naked)]
unsafe extern "C" fn syscall_entry_naked() {
    naked_asm!(
        // Save user context to kernel stack for iretq return
        "push qword ptr [{ss_seg}]",   // SS (user data segment)
        "push r11",                     // RFLAGS (saved by syscall)
        "push rcx",                     // RIP (saved by syscall)
        "push qword ptr [{cs_seg}]",   // CS (user code segment)

        // Save user stack pointer
        "push rsp",                     // save user RSP (will be overwritten by push below)

        // Save all general-purpose registers
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

        // Load kernel stack
        "mov r15, rsp",                 // save pointer to saved context
        "mov rsp, [rip + {kstack}]",    // load kernel stack

        // Call Rust handler with pointer to saved context
        "mov rdi, r15",                 // arg 0: pointer to InterruptFrame
        "call {handler}",

        // Restore kernel stack pointer
        "mov rsp, r15",

        // Restore all general-purpose registers
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

        // Restore user stack pointer
        "pop rsp",

        // Restore CS, RIP, RFLAGS, SS for iretq
        "add rsp, 8",                   // skip CS (already in segment registers)
        "pop rcx",                      // RIP
        "pop r11",                      // RFLAGS
        "add rsp, 8",                   // skip SS (already in segment registers)

        // Return to Ring 3 via iretq
        "iretq",

        kstack = sym SYSCALL_KERNEL_RSP,
        handler = sym syscall_handler_rust,
        ss_seg = const 0x1B_u64,        // USER_DS | RPL=3
        cs_seg = const 0x23_u64,        // USER_CS | RPL=3
    );
}

static mut RING3_SYSCALL_SEEN: bool = false;

/// Rust syscall handler — dispatches by syscall number.
///
/// `frame` points to the saved user context on the kernel stack.
/// On return, the frame will be restored and iretq will return to Ring 3.
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
        let _a5 = f.r9; // 7th arg (BMO ABI) — reserved for future use

        crate::diag::trace_u64("syscall", "dispatch nr", nr);

        let result = match nr {
            // ─── Procesos ─────────────────────────────────────────────
            // ProcessExit (0x00) — kill current process and return to scheduler
            0x00 => {
                crate::diag::trace("syscall", "ProcessExit");
                // Kill current process — never returns
                crate::sched::process::kill_current_process(0, a0, 0);
            }

            // ThreadCreate (0x01) — not implemented yet
            0x01 => u64::MAX,

            // ThreadExit (0x02) — not implemented yet
            0x02 => u64::MAX,

            // ThreadYield (0x03)
            0x03 => {
                crate::sched::yield_now();
                0
            }

            // FutexWait (0x04) — not implemented yet
            0x04 => u64::MAX,

            // FutexWake (0x05) — not implemented yet
            0x05 => u64::MAX,

            // ─── Memoria ──────────────────────────────────────────────
            // Mmap (0x10) — not implemented yet
            0x10 => u64::MAX,

            // Munmap (0x11) — not implemented yet
            0x11 => u64::MAX,

            // Mprotect (0x12) — not implemented yet
            0x12 => u64::MAX,

            // ─── VFS ──────────────────────────────────────────────────
            // FileOpen (0x20): a0=name_ptr, a1=name_len → fd or u64::MAX
            0x20 => crate::fs::ramdisk::open(a0, a1),

            // FileRead (0x21): a0=fd, a1=ptr, a2=len → bytes read
            0x21 => crate::fs::ramdisk::read(a0, a1, a2),

            // FileWrite (0x22) — not implemented yet
            0x22 => u64::MAX,

            // FileClose (0x23): a0=fd → 0 or u64::MAX
            0x23 => crate::fs::ramdisk::close(a0),

            // FileSeek (0x24) — not implemented yet
            0x24 => u64::MAX,

            // FileStat (0x25): a0=fd → bytes total
            0x25 => crate::fs::ramdisk::size(a0),

            // ─── IPC ──────────────────────────────────────────────────
            // PortCreate (0x30) — not implemented yet
            0x30 => u64::MAX,

            // PortSend (0x31) — not implemented yet
            0x31 => u64::MAX,

            // PortRecv (0x32) — not implemented yet
            0x32 => u64::MAX,

            // ─── BareX bridges ────────────────────────────────────────
            // BarexGfxSubmit (0x40) — not implemented yet
            0x40 => u64::MAX,

            // BarexAudioSubmit (0x41) — not implemented yet
            0x41 => u64::MAX,

            // BarexInputPoll (0x42) — not implemented yet
            0x42 => u64::MAX,

            // BarexNetSubmit (0x43) — not implemented yet
            0x43 => u64::MAX,

            // ─── Tiempo ───────────────────────────────────────────────
            // ClockGetTime (0x50): returns rdtsc value
            0x50 => crate::arch::cpu::rdtsc(),

            // NanoSleep (0x51): a0 = nanoseconds (busy-wait)
            0x51 => {
                let target_ns = a0;
                let target_cycles = (target_ns as u128 * 37) / 10; // ~3.7 GHz
                let start = crate::arch::cpu::rdtsc();
                while (crate::arch::cpu::rdtsc() - start) < target_cycles as u64 {
                    core::hint::spin_loop();
                }
                0
            }

            // ─── Framebuffer (compositor Ring 3) ──────────────────────
            // FbInfo (0x60): no args, returns packed:
            //   bits  0..31 = width  | bits 32..47 = height | bits 48..63 = stride/4
            0x60 => {
                let w = crate::boot_info::FB_WIDTH as u64;
                let h = crate::boot_info::FB_HEIGHT as u64;
                let s = (crate::boot_info::FB_STRIDE / 1) as u64;
                w | (h << 32) | ((s & 0xFFFF) << 48)
            }

            // FbFill (0x61): a0=x, a1=y, a2=w, a3=h, a4=color (0xAARRGGBB)
            0x61 => {
                crate::desktop::fb_fill(a0 as u32, a1 as u32, a2 as u32, a3 as u32, a4 as u32);
                0
            }

            // FbText (0x62): a0=x, a1=y, a2=ptr_utf8, a3=len, a4=color
            0x62 => {
                if a3 > 0 && a3 < 256 {
                    let slice = core::slice::from_raw_parts(a2 as *const u8, a3 as usize);
                    crate::desktop::fb_text(a0 as u32, a1 as u32, slice, a4 as u32);
                }
                0
            }

            // FbPresent (0x63): no-op (direct framebuffer writes)
            0x63 => 0,

            // FbBlit (0x64): a0=x, a1=y, a2=w, a3=h, a4=src_ptr (XRGB-8888 raster)
            0x64 => {
                crate::desktop::fb_blit(a0 as u32, a1 as u32, a2 as u32, a3 as u32, a4);
                0
            }

            // DesktopFrame (0x65): renderiza un frame completo del escritorio
            0x65 => {
                crate::desktop::render::render_frame();
                crate::diag::paint_overlay();
                crate::desktop::state::STATE.frame
            }

            // ─── Input ────────────────────────────────────────────────
            // KeyPoll (0x70): returns PS/2 scancode or 0 if no key
            0x70 => crate::desktop::poll_key() as u64,

            // MousePoll (0x71): returns x:i16 | y:i16<<16 | buttons<<32
            0x71 => crate::desktop::poll_mouse(),

            // ─── Sonido ───────────────────────────────────────────────
            // Beep (0x80): a0=freq_hz, a1=duration_ms
            0x80 => {
                crate::desktop::beep(a0 as u32, a1 as u32);
                0
            }

            // ─── Debug ────────────────────────────────────────────────
            // DebugPrint (0xF0): a0=ptr_utf8, a1=length → serial out
            0xF0 => {
                if a1 > 0 && a1 < 4096 {
                    let slice = core::slice::from_raw_parts(a0 as *const u8, a1 as usize);
                    if let Ok(s) = core::str::from_utf8(slice) {
                        crate::drivers::serial::serial_write(s);
                    }
                }
                0
            }

            // Unknown syscall
            _ => {
                crate::diag::warn_u64("syscall", "unknown syscall", nr);
                u64::MAX
            }
        };

        // Store result in RAX for return to Ring 3
        f.rax = result;
    }
}
