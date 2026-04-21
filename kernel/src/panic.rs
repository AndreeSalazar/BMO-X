//! Panic handler — serial output (no VGA text mode).

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // Output to serial COM1 (0x3F8) — works regardless of display mode
    let msg = b"\r\n!!! KERNEL PANIC !!!\r\n";
    for &b in msg {
        unsafe {
            // Wait for transmit ready
            while (port_read(0x3FD) & 0x20) == 0 {}
            port_write(0x3F8, b);
        }
    }
    loop { unsafe { core::arch::asm!("cli"); core::arch::asm!("hlt"); } }
}

#[inline]
fn port_write(port: u16, val: u8) {
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val); }
}

#[inline]
fn port_read(port: u16) -> u8 {
    let v: u8;
    unsafe { core::arch::asm!("in al, dx", out("al") v, in("dx") port); }
    v
}
