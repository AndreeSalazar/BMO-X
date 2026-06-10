//! Syscall Entry Point — IA32_LSTAR / STAR / FMASK MSR setup.
//!
//! x86-64 `syscall` instruction:
//!   - Saves RIP in RCX, RFLAGS in R11
//!   - Loads CS from STAR[47:32], SS from STAR[47:32]+8
//!   - Loads RIP from IA32_LSTAR
//!   - Masks RFLAGS with IA32_FMASK
//!
//! x86-64 `sysret` instruction:
//!   - Restores RIP from RCX, RFLAGS from R11
//!   - Loads CS from STAR[63:48]+16, SS from STAR[63:48]+8
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

/// Naked syscall entry point — called by hardware via IA32_LSTAR.
///
/// On entry from `syscall`:
///   RCX = saved RIP (user return address)
///   R11 = saved RFLAGS
///   RAX = syscall number
///   RDI, RSI, RDX, R10, R8, R9 = arguments
///   RSP = still user RSP! We must switch to kernel stack.
#[unsafe(naked)]
unsafe extern "C" fn syscall_entry_naked() {
    naked_asm!(
        // Swap to kernel stack: save user RSP in scratch register,
        // load kernel RSP from TSS.rsp[0] via swapgs + gs:offset.
        // Simpler approach: use a known kernel stack location.
        //
        // Save user RSP, load kernel RSP
        "mov r15, rsp",              // save user RSP in r15 (callee-saved, we'll restore)

        // Load kernel RSP from a fixed location (set by set_kernel_stack)
        // We use swapgs to access per-CPU data, but for single-CPU we use a global.
        "mov rsp, [rip + {kstack}]",

        // Build SyscallFrame on kernel stack
        "push r15",                   // user RSP
        "push r11",                   // user RFLAGS
        "push rcx",                   // user RIP

        // Save callee-saved registers we use
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // Call Rust handler: syscall_handler(nr, arg0-5, user_rsp, user_rip)
        // Syscall ABI: RAX=nr, RDI=a0, RSI=a1, RDX=a2, R10=a3, R8=a4, R9=a5
        // C ABI: RDI=a0, RSI=a1, RDX=a2, RCX=a3, R8=a4, R9=a5
        // We need: fn handler(nr: u64, a0-a5: u64) -> u64
        // Rearrange args for C calling convention:
        "mov rcx, r10",              // 4th arg: R10 → RCX (C ABI)
        // RDI, RSI, RDX already correct
        // R8, R9 already correct
        // Push syscall number as 1st arg, shift others
        "push r9",                    // save r9 (will be 7th arg on stack)
        "push r8",                    // save r8
        "push rcx",                   // save original r10 (now in rcx)
        "push rdx",                   // save rdx
        "push rsi",                   // save rsi
        "push rdi",                   // save rdi
        "mov rdi, rax",              // 1st arg = syscall number
        "pop rsi",                    // 2nd arg = original rdi (user a0)
        "pop rdx",                    // 3rd arg = original rsi (user a1)
        "pop rcx",                    // 4th arg = original rdx (user a2)
        "pop r8",                     // 5th arg = original r10 (user a3)
        "pop r9",                     // 6th arg = original r8  (user a4)
        // 7th arg (original r9) is on stack — we'll ignore for now (6 args max)
        "pop r10",                    // clean stack (was r9)

        "call {handler}",

        // RAX now has return value

        // Restore callee-saved regs
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",

        // Restore user RIP → RCX, user RFLAGS → R11, user RSP
        "pop rcx",                    // user RIP
        "pop r11",                    // user RFLAGS
        "pop rsp",                    // user RSP (directly restore)

        // Return to Ring 3
        "sysretq",

        kstack = sym SYSCALL_KERNEL_RSP,
        handler = sym syscall_handler_rust,
    );
}

/// Kernel RSP for syscall entry. Updated by set_syscall_kernel_stack().
#[unsafe(no_mangle)]
static mut SYSCALL_KERNEL_RSP: u64 = 0;

/// Set the kernel stack pointer used by the syscall entry trampoline.
pub fn set_syscall_kernel_stack(rsp: u64) {
    unsafe { SYSCALL_KERNEL_RSP = rsp; }
}

/// Rust syscall handler — dispatches by syscall number.
///
/// Args follow BMO ABI: nr in RAX, then RDI, RSI, RDX, R10, R8, R9.
/// After register shuffling: nr=rdi, a0=rsi, a1=rdx, a2=rcx, a3=r8, a4=r9.
#[unsafe(no_mangle)]
extern "C" fn syscall_handler_rust(
    nr: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
) -> u64 {
    match nr {
        // ─── Procesos ─────────────────────────────────────────────
        // ProcessExit (0x00) — halt CPU
        0x00 => loop { unsafe { core::arch::asm!("hlt"); } },

        // ThreadYield (0x03)
        0x03 => { core::hint::spin_loop(); 0 }

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
            unsafe {
                let w = crate::boot_info::FB_WIDTH as u64;
                let h = crate::boot_info::FB_HEIGHT as u64;
                let s = (crate::boot_info::FB_STRIDE / 1) as u64;
                w | (h << 32) | ((s & 0xFFFF) << 48)
            }
        }

        // FbFill (0x61): a0=x, a1=y, a2=w, a3=h, a4=color (0xAARRGGBB)
        0x61 => {
            crate::desktop::fb_fill(a0 as u32, a1 as u32, a2 as u32, a3 as u32, a4 as u32);
            0
        }

        // FbText (0x62): a0=x, a1=y, a2=ptr_utf8, a3=len, a4=color
        0x62 => {
            if a3 > 0 && a3 < 256 {
                let slice = unsafe {
                    core::slice::from_raw_parts(a2 as *const u8, a3 as usize)
                };
                crate::desktop::fb_text(a0 as u32, a1 as u32, slice, a4 as u32);
            }
            0
        }

        // FbPresent (0x63): no-op (direct framebuffer writes); reserved for future double-buffer flip.
        0x63 => 0,

        // FbBlit (0x64): a0=x, a1=y, a2=w, a3=h, a4=src_ptr (XRGB-8888 raster)
        0x64 => {
            crate::desktop::fb_blit(a0 as u32, a1 as u32, a2 as u32, a3 as u32, a4);
            0
        }

        // DesktopFrame (0x65): renderiza un frame completo del escritorio
        //   (wallpaper + status bar + ventanas + dock + cursor) en Ring 0.
        //   Sin args. Devuelve frame counter.
        0x65 => {
            crate::desktop::render::render_frame();
            unsafe { crate::desktop::state::STATE.frame }
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

        // ─── Filesystem (RAMdisk) ─────────────────────────────────
        // FileOpen (0x20): a0=name_ptr, a1=name_len → fd or u64::MAX
        0x20 => crate::fs::ramdisk::open(a0, a1),
        // FileRead (0x21): a0=fd, a1=ptr, a2=len → bytes read
        0x21 => crate::fs::ramdisk::read(a0, a1, a2),
        // FileClose (0x23): a0=fd → 0 or u64::MAX
        0x23 => crate::fs::ramdisk::close(a0),
        // FileSize (0x25): a0=fd → bytes total
        0x25 => crate::fs::ramdisk::size(a0),

        // ─── Debug ────────────────────────────────────────────────
        // DebugPrint (0xF0): a0=ptr_utf8, a1=length → serial out
        0xF0 => {
            if a1 > 0 && a1 < 4096 {
                let slice = unsafe {
                    core::slice::from_raw_parts(a0 as *const u8, a1 as usize)
                };
                if let Ok(s) = core::str::from_utf8(slice) {
                    crate::drivers::serial::serial_write(s);
                }
            }
            0
        }

        // Unknown syscall
        _ => u64::MAX,
    }
}
