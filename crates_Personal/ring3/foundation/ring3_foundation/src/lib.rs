//! Ring 3 Foundation — syscall wrappers + BMO Channel client.
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

use bmo_channel::{Channel, ChannelEntry};

// ═══════════════════════════════════════════════════════════════════════
//  Syscall primitive — the ONLY raw syscall in Ring 3
// ═══════════════════════════════════════════════════════════════════════

/// Execute a syscall. This is the single point where we enter Ring 0.
/// All other `sys_*` functions are safe wrappers around this.
unsafe fn syscall0(nr: u64) -> u64 {
    let result: u64;
    core::arch::asm!(
        "syscall",
        inlateout("rax") nr => result,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack),
    );
    result
}

unsafe fn syscall1(nr: u64, a0: u64) -> u64 {
    let result: u64;
    core::arch::asm!(
        "syscall",
        inlateout("rax") nr => result,
        in("rdi") a0,
        lateout("rcx") _,
        lateout("r11") _,
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
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack),
    );
    result
}

// ═══════════════════════════════════════════════════════════════════════
//  Type-safe syscall wrappers
// ═══════════════════════════════════════════════════════════════════════

/// Register a BMO Channel page with the kernel.
pub fn sys_channel_register(phys: u64) -> bool {
    unsafe { syscall1(0x34, phys) == 0 }
}

/// Poll a BMO Channel for responses (immediate, no timer tick wait).
pub fn sys_channel_poll(phys: u64) -> usize {
    unsafe { syscall1(0x35, phys) as usize }
}

/// Force immediate channel processing.
pub fn sys_channel_kick() -> usize {
    unsafe { syscall0(0x36) as usize }
}

/// Play a beep via PC speaker.
pub fn sys_beep(freq: u32, duration_ms: u32) {
    unsafe { syscall2(0x37, freq as u64, duration_ms as u64); }
}

/// Read from an I/O port.
pub fn sys_port_in(port: u16) -> u8 {
    unsafe { syscall1(0x70, port as u64) as u8 }
}

/// Write to an I/O port.
pub fn sys_port_out(port: u16, value: u8) {
    unsafe { syscall2(0x71, port as u64, value as u64); }
}

/// Get current TSC.
pub fn sys_clock_get() -> u64 {
    unsafe { syscall0(0x50) }
}

/// Sleep for nanoseconds (busy-wait).
pub fn sys_nanosleep(ns: u64) {
    unsafe { syscall1(0x51, ns); }
}

// ═══════════════════════════════════════════════════════════════════════
//  BMO Channel client — Ring 3 side
// ═══════════════════════════════════════════════════════════════════════

/// Client for a BMO Channel shared page.
/// Wraps raw syscalls for register/poll/kick.
pub struct ChannelClient {
    phys: u64,
    virt: *const Channel,
}

impl ChannelClient {
    /// Connect to the system channel at the given physical address.
    /// The channel must already be initialized by the kernel.
    pub fn connect_system(sys_channel_phys: u64) -> Self {
        // On BMO, physical == virtual (identity-mapped page tables)
        let virt = sys_channel_phys as *const Channel;
        Self { phys: sys_channel_phys, virt }
    }

    /// Register a new user channel (Ring 3 allocates the page).
    pub fn register(phys: u64) -> Option<Self> {
        if sys_channel_register(phys) {
            Some(Self { phys, virt: phys as *const Channel })
        } else {
            None
        }
    }

    /// Get a reference to the channel.
    pub fn channel(&self) -> &Channel {
        unsafe { &*self.virt }
    }

    /// Poll for completed events. Returns number consumed.
    pub fn poll(&self) -> usize {
        self.channel().ring3_poll(|_, _, _, _| {})
    }

    /// Poll with callback. Returns number consumed.
    pub fn poll_with<F: FnMut(u64, u64, u64, u64)>(&self, callback: F) -> usize {
        self.channel().ring3_poll(callback)
    }

    /// Submit an event to the channel and signal the kernel.
    pub fn send(&self, opcode: u64, arg0: u64, arg1: u64, arg2: u64) -> bool {
        self.channel().ring3_send(opcode, arg0, arg1, arg2)
    }

    /// Force immediate processing via syscall.
    pub fn kick(&self) -> usize {
        sys_channel_kick()
    }

    /// Physical address of the channel (for sharing).
    pub fn phys(&self) -> u64 {
        self.phys
    }
}
