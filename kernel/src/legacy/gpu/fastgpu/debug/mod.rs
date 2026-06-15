//! Debug and Evidence Logging
//! Structured tracing for hardware provenance and transitions.

use core::fmt;

pub struct SerialWriter;

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        crate::drivers::serial::serial_write(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! evidence_println {
    ($($arg:tt)*) => {
        let _ = core::fmt::Write::write_fmt(&mut $crate::drivers::gpu::fastgpu::debug::SerialWriter, core::format_args!($($arg)*));
        $crate::drivers::serial::serial_write("\n");
    };
}

pub struct RegisterSnapshot {
    pub timestamp: u64,
    pub register: u32,
    pub value: u32,
}

impl RegisterSnapshot {
    pub fn new(register: u32, value: u32) -> Self {
        Self {
            timestamp: 0, // In FastOS we don't have a solid timer yet, use 0 or RDTSC
            register,
            value,
        }
    }
}
