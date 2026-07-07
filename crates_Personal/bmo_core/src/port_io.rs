//! Port I/O primitives — single source of truth for `inb`/`outb`.
//!
//! Used by: console, audio, watchdog, desktop input, HDA, etc.

#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack));
    val
}

#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}

#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let val: u16;
    core::arch::asm!("in ax, dx", out("ax") val, in("dx") port, options(nomem, nostack));
    val
}

#[inline]
pub unsafe fn outw(port: u16, val: u16) {
    core::arch::asm!("out dx, ax", in("dx") port, in("ax") val, options(nomem, nostack));
}

/// Wait for PS/2 input buffer to be empty (bit 1 = 0).
#[inline]
pub unsafe fn ps2_wait_input() {
    while (inb(0x64) & 0x02) != 0 { core::hint::spin_loop(); }
}

/// Wait for PS/2 output buffer to be full (bit 0 = 1).
#[inline]
pub unsafe fn ps2_wait_output() {
    let mut timeout = 100000;
    while (inb(0x64) & 0x01) == 0 && timeout > 0 {
        timeout -= 1;
        core::hint::spin_loop();
    }
}

/// System reset via keyboard controller (pulse CPU reset line).
pub fn system_reset() -> ! {
    unsafe {
        ps2_wait_input();
        outb(0x64, 0xFE);
    }
    loop { unsafe { core::arch::asm!("hlt"); } }
}
