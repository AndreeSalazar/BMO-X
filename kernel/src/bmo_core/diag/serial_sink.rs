//! Salida serial COM1 para diag/.

use super::event::{severity_name, Event};
use crate::dev::console;

pub(crate) fn write_event(event: Event) {
    console::serial_write("[DIAG][");
    console::serial_write(severity_name(event.severity));
    console::serial_write("][");
    console::serial_write(event.module);
    console::serial_write("] ");
    console::serial_write(event.message);
    if event.has_value {
        console::serial_write(" = ");
        serial_hex(event.value);
    }
    console::serial_write("\n");
}

fn serial_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    console::serial_write("0x");
    for i in (0..16).rev() {
        console::serial_write_byte(hex[((val >> (i * 4)) & 0xF) as usize]);
    }
}
