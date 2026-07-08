//! Ring 0 Logger — serial-only logging for boot phases.
//! No visual, no cabina — just serial output.

use crate::dev::console;

pub fn info(_phase: &'static str, msg: &'static str) {
    console::serial_write("[BMO] ");
    console::serial_write(msg);
    console::serial_write("\n");
}

pub fn warn(_phase: &'static str, msg: &'static str) {
    console::serial_write("[BMO] WARN: ");
    console::serial_write(msg);
    console::serial_write("\n");
}

pub fn fault(_phase: &'static str, msg: &'static str) -> ! {
    console::serial_write("[BMO] FATAL: ");
    console::serial_write(msg);
    console::serial_write("\n");
    unsafe { core::arch::asm!("cli"); }
    loop { unsafe { core::arch::asm!("hlt"); } }
}

pub fn info_u64(_phase: &'static str, msg: &'static str, val: u64) {
    console::serial_write("[BMO] ");
    console::serial_write(msg);
    console::serial_write(": ");
    console::serial_write_u64(val, 10);
    console::serial_write("\n");
}
