//! Ring 3 Foundation — syscall wrappers.
//!
//! This is the ONLY crate in Ring 3 that makes raw syscalls.
//! Every other crate uses these type-safe wrappers.
//!
//! ## Architecture invariant
//!
//! ```text
//! ring3_foundation  ←  ONLY place with `asm!("syscall")`
//!       ↑
//!   all other crates (drivers, services, desktop)
//! ```

#![no_std]

extern crate alloc;

// ═══════════════════════════════════════════════════════════════════════
//  Syscall primitives
// ═══════════════════════════════════════════════════════════════════════

unsafe fn syscall0(nr: u64) -> u64 {
    let result: u64;
    core::arch::asm!(
        "syscall",
        inlateout("rax") nr => result,
        // syscall clobbers RCX (rip) and R11 (rflags)
        // Kernel handler clobbers caller-saved regs (rdi,rsi,rdx,r8,r9,r10)
        lateout("rcx") _,
        lateout("r11") _,
        out("rdi") _, out("rsi") _, out("rdx") _,
        out("r8") _, out("r9") _, out("r10") _,
        options(nostack),
    );
    result
}

unsafe fn syscall2(nr: u64, a0: u64, a1: u64) -> u64 {
    let result: u64;
    core::arch::asm!(
        "syscall",
        inlateout("rax") nr => result,
        in("rdi") a0,
        in("rsi") a1,
        // syscall clobbers RCX (rip) and R11 (rflags)
        lateout("rcx") _,
        lateout("r11") _,
        out("rdx") _, out("r8") _, out("r9") _, out("r10") _,
        options(nostack),
    );
    result
}

// ═══════════════════════════════════════════════════════════════════════
//  Input syscalls
// ═══════════════════════════════════════════════════════════════════════

/// Poll the system channel for next keyboard scancode.
/// Returns scancode (0-255) or 0 if no events.
/// Bit 7 set = key released.
pub fn sys_keyboard_poll() -> u8 {
    unsafe { syscall0(0x38) as u8 }
}

/// Poll the system channel for next mouse event.
/// Returns packed: (buttons << 32) | (dy << 16) | dx.
/// Returns u64::MAX if no events.
pub fn sys_mouse_poll() -> u64 {
    unsafe { syscall0(0x39) }
}

// ═══════════════════════════════════════════════════════════════════════
//  Other syscalls
// ═══════════════════════════════════════════════════════════════════════

/// Play a beep via PC speaker.
pub fn sys_beep(freq: u32, duration_ms: u32) {
    unsafe { syscall2(0x37, freq as u64, duration_ms as u64); }
}

/// Force immediate channel processing.
pub fn sys_channel_kick() -> usize {
    unsafe { syscall0(0x36) as usize }
}

/// Reboot the system.
pub fn sys_reboot() -> ! {
    unsafe { syscall0(0x3A); }
    loop { unsafe { core::arch::asm!("hlt"); } }
}

/// Shutdown the system.
pub fn sys_shutdown() -> ! {
    unsafe { syscall0(0x3B); }
    loop { unsafe { core::arch::asm!("hlt"); } }
}
