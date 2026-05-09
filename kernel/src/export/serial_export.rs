//! Serial JSON Exporter — sends the spy report via COM1 at 115200 baud.
//!
//! Uses delimited output so the host can easily capture the JSON.

use crate::drivers::serial;

const CHUNK_SIZE: usize = 512;

/// Export a JSON string via serial with clear delimiters.
pub fn export_json_serial(json: &str) {
    serial::serial_write("\r\n");
    serial::serial_write("===== FASTOS SPY REPORT BEGIN =====\r\n");

    // Send in chunks to avoid UART FIFO overflow
    let bytes = json.as_bytes();
    let mut offset = 0;
    while offset < bytes.len() {
        let end = (offset + CHUNK_SIZE).min(bytes.len());
        for &b in &bytes[offset..end] {
            serial::serial_write_byte(b);
        }
        offset = end;

        // Small spin delay between chunks to let UART drain
        for _ in 0..5000 {
            core::hint::spin_loop();
        }
    }

    serial::serial_write("\r\n===== FASTOS SPY REPORT END =====\r\n");
}
