//! Salida serial COM1 para diag/.

use super::event::{severity_name, Event};
use crate::drivers::serial;

pub(crate) fn write_event(event: Event) {
    serial::serial_write("[DIAG][");
    serial::serial_write(severity_name(event.severity));
    serial::serial_write("][");
    serial::serial_write(event.module);
    serial::serial_write("] ");
    serial::serial_write(event.message);
    if event.has_value {
        serial::serial_write(" = ");
        serial_hex(event.value);
    }
    serial::serial_write("\n");
}

fn serial_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    serial::serial_write("0x");
    for i in (0..16).rev() {
        serial::serial_write_byte(hex[((val >> (i * 4)) & 0xF) as usize]);
    }
}
