//! Data export utilities for the forensic agent.

use crate::fs::DiskError;

pub trait DiskWriter {
    fn write_sectors(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<(), DiskError>;
}

pub struct UsbWriter<W: DiskWriter> {
    writer: W,
}

impl<W: DiskWriter> UsbWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn write_file(&mut self, name: &str, data: &[u8]) -> Result<(), DiskError> {
        // Placeholder para la lógica FAT32.
        // Usamos serial para loggear el progreso.
        crate::drivers::serial::serial_write("[USB] Exporting file...\n");
        // Simular escritura exitosa
        let _ = name;
        let _ = data;
        Ok(())
    }
}
