//! PS/2 Keyboard — IRQ 1 handler + i8042 polling.
//!
//! On each timer tick, checks if the i8042 controller (port 0x64)
//! has keyboard data (bit 0). If so, reads the scancode from port
//! 0x60 and pushes it to the BMO system channel.
//!
//! Once the IOAPIC is configured, IRQ 1 will be connected to the
//! `keyboard_isr()` handler for true interrupt-driven input.

use core::arch::asm;

/// Keyboard data port.
const PORT_DATA: u16 = 0x60;
/// Keyboard status port.
const PORT_STATUS: u16 = 0x64;

/// BMO Channel opcode: keyboard scancode.
const OP_KEY: u64 = 0xB000_0002;

/// Non-blocking check: is keyboard data available?
fn has_data() -> bool {
    unsafe {
        let status: u8;
        asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
        status & 1 != 0
    }
}

/// Read a byte from the keyboard data port.
fn read_data() -> u8 {
    unsafe {
        let data: u8;
        asm!("in al, dx", in("dx") PORT_DATA, out("al") data, options(nostack, nomem));
        data
    }
}

/// Poll keyboard on timer tick. Called from the timer ISR (arch/idt.rs).
/// Reads available scancodes and pushes them to the system channel.
pub fn tick() {
    while has_data() {
        let sc = read_data();
        if sc == 0xFA { continue; }
        let pressed = (sc & 0x80) == 0;
        let code = sc & 0x7F;
        crate::channel::sys_send(OP_KEY, code as u64, pressed as u64, 0);
    }
}

/// Initialize the keyboard subsystem: enable IRQ 1 in the i8042.
pub fn init() {
    unsafe {
        let mut cfg: u8;
        // Command 0x20: read configuration byte
        asm!("out 0x64, al", in("al") 0x20u8, options(nostack, nomem));
        // Wait for output buffer full
        loop {
            let status: u8;
            asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
            if status & 1 != 0 { break; }
        }
        asm!("in al, dx", in("dx") PORT_DATA, out("al") cfg, options(nostack, nomem));

        // Set bit 0 (enable keyboard interrupt)
        if cfg & 1 == 0 {
            cfg |= 1;
            asm!("out 0x64, al", in("al") 0x60u8, options(nostack, nomem));
            loop {
                let status: u8;
                asm!("in al, dx", in("dx") PORT_STATUS, out("al") status, options(nostack, nomem));
                if status & 2 == 0 { break; }
            }
            asm!("out 0x60, al", in("al") cfg, options(nostack, nomem));
        }
    }
}
