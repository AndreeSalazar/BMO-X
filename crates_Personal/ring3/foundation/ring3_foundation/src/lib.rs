//! Ring 3 Foundation — syscall wrappers.
//!
//! This is the ONLY crate in Ring 3 that makes raw syscalls.
//! Every other crate uses these type-safe wrappers.
//!
//! ## Syscall coverage: 19 of 28 kernel syscalls wrapped
//!
//! Missing (feature-gated): IPC (0x30-0x31), NET (0x90), GPU (0xA0)
//! Missing (stubs): munmap (0x11), mprotect (0x12), fb_map (0x61), fb_flush (0x62)

#![no_std]

extern crate alloc;

// ═══════════════════════════════════════════════════════════════════════
//  Syscall primitives (syscall0..syscall6)
// ═══════════════════════════════════════════════════════════════════════

unsafe fn syscall0(nr: u64) -> u64 {
    let result: u64;
    core::arch::asm!(
        "syscall",
        inlateout("rax") nr => result,
        lateout("rcx") _, lateout("r11") _,
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
        in("rdi") a0, in("rsi") a1,
        lateout("rcx") _, lateout("r11") _,
        out("rdx") _, out("r8") _, out("r9") _, out("r10") _,
        options(nostack),
    );
    result
}

unsafe fn syscall4(nr: u64, a0: u64, a1: u64, a2: u64, a3: u64) -> u64 {
    let result: u64;
    core::arch::asm!(
        "syscall",
        inlateout("rax") nr => result,
        in("rdi") a0, in("rsi") a1, in("rdx") a2, in("r10") a3,
        lateout("rcx") _, lateout("r11") _,
        out("r8") _, out("r9") _,
        options(nostack),
    );
    result
}

// ═══════════════════════════════════════════════════════════════════════
//  Input / Channel (0x34-0x39)
// ═══════════════════════════════════════════════════════════════════════

pub fn sys_keyboard_poll() -> u8 {
    unsafe { syscall0(0x38) as u8 }
}

pub fn sys_mouse_poll() -> u64 {
    unsafe { syscall0(0x39) }
}

pub fn sys_beep(freq: u32, duration_ms: u32) {
    unsafe { syscall2(0x37, freq as u64, duration_ms as u64); }
}

pub fn sys_channel_kick() -> usize {
    unsafe { syscall0(0x36) as usize }
}

// ═══════════════════════════════════════════════════════════════════════
//  Time (0x50-0x51)
// ═══════════════════════════════════════════════════════════════════════

pub fn sys_clock_get() -> u64 {
    unsafe { syscall0(0x50) }
}

pub fn sys_nanosleep(ns: u64) {
    unsafe { syscall2(0x51, ns, 0); }
}

pub fn sys_sleep_ms(ms: u64) {
    sys_nanosleep(ms * 1_000_000);
}

// ═══════════════════════════════════════════════════════════════════════
//  Framebuffer (0x60)
// ═══════════════════════════════════════════════════════════════════════

pub fn sys_fb_info() -> (u64, u32, u32, u32) {
    let packed = unsafe { syscall0(0x60) };
    let addr = packed & 0xFFFF_FFFF_FFFF;
    let w = ((packed >> 32) & 0xFFFF) as u32;
    let h = ((packed >> 48) & 0xFFFF) as u32;
    let stride = ((packed >> 56) & 0xFF) as u32;
    (addr, w, h, stride)
}

// ═══════════════════════════════════════════════════════════════════════
//  Port I/O (0x70-0x71)
// ═══════════════════════════════════════════════════════════════════════

pub fn sys_port_in(port: u16) -> u8 {
    unsafe { syscall2(0x70, port as u64, 0) as u8 }
}

pub fn sys_port_out(port: u16, value: u8) {
    unsafe { syscall2(0x71, port as u64, value as u64); }
}

// ═══════════════════════════════════════════════════════════════════════
//  Memory (0x10)
// ═══════════════════════════════════════════════════════════════════════

pub fn sys_mmap(phys: u64, virt: u64, pages: u64, flags: u64) -> bool {
    unsafe { syscall4(0x10, phys, virt, pages, flags) == 0 }
}

// ═══════════════════════════════════════════════════════════════════════
//  Debug (0xF0)
// ═══════════════════════════════════════════════════════════════════════

pub fn sys_debug_print(msg: &str) {
    unsafe { syscall2(0xF0, msg.as_ptr() as u64, msg.len() as u64); }
}

// ═══════════════════════════════════════════════════════════════════════
//  Power (0x3A-0x3B)
// ═══════════════════════════════════════════════════════════════════════

pub fn sys_reboot() -> ! {
    unsafe { syscall0(0x3A); }
    loop { unsafe { core::arch::asm!("hlt"); } }
}

pub fn sys_shutdown() -> ! {
    unsafe { syscall0(0x3B); }
    loop { unsafe { core::arch::asm!("hlt"); } }
}
