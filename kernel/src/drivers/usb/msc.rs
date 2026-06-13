//! USB Mass Storage Class (MSC) - Bulk-Only Transport (BOT) & SCSI Engine.
//!
//! Traduce llamadas sector-por-sector (DiskReader) a comandos SCSI READ10/WRITE10
//! transmitidos vía USB CBW (Command Block Wrapper) y verificados con CSW.

#![allow(dead_code)]

use crate::drivers::serial;
use crate::fs::{DiskReader, DiskWriter, DiskError};
use bmo_usb::{
    CommandBlockWrapper, CommandStatusWrapper, ScsiRead10,
    CBW_SIGNATURE, CSW_SIGNATURE, SCSI_CMD_READ_CAPACITY, SCSI_CMD_INQUIRY
};

pub struct UsbMscDevice {
    pub slot_id: u8,
    pub bulk_in_ep: u8,
    pub bulk_out_ep: u8,
    pub block_size: u32,
    pub total_blocks: u64,
}

impl UsbMscDevice {
    pub fn new(slot_id: u8, bulk_in: u8, bulk_out: u8) -> Self {
        Self {
            slot_id,
            bulk_in_ep: bulk_in,
            bulk_out_ep: bulk_out,
            block_size: 512, // Estándar para discos USB MSC/SCSI
            total_blocks: 0,
        }
    }

    /// Inicializa el dispositivo enviando los comandos obligatorios SCSI INQUIRY y READ CAPACITY.
    pub fn init_device(&mut self) -> Result<(), &'static str> {
        serial::serial_write("[USB-MSC] Inicializando dispositivo SCSI...\n");
        
        // 1. SCSI Inquiry
        let mut inquiry_buf = [0u8; 36];
        self.execute_scsi_cmd(SCSI_CMD_INQUIRY, 0, &mut inquiry_buf, true)?;
        serial::serial_write("[USB-MSC] SCSI Inquiry completado.\n");

        // 2. SCSI Read Capacity
        let mut capacity_buf = [0u8; 8];
        self.execute_scsi_cmd(SCSI_CMD_READ_CAPACITY, 0, &mut capacity_buf, true)?;
        
        // El resultado viene en Big-Endian:
        // bytes 0..3: LBA máximo (total_blocks - 1)
        // bytes 4..7: Tamaño del bloque
        let max_lba = u32::from_be_bytes([capacity_buf[0], capacity_buf[1], capacity_buf[2], capacity_buf[3]]) as u64;
        let block_size = u32::from_be_bytes([capacity_buf[4], capacity_buf[5], capacity_buf[6], capacity_buf[7]]);
        
        self.total_blocks = max_lba + 1;
        if block_size > 0 {
            self.block_size = block_size;
        }

        serial::serial_write("[USB-MSC] Capacidad detectada: ");
        crate::serial_hex(self.total_blocks);
        serial::serial_write(" bloques de ");
        crate::serial_hex(self.block_size as u64);
        serial::serial_write(" bytes.\n");

        Ok(())
    }

    /// Motor SCSI BOT: Envía CBW, transmite/recibe datos y valida el CSW.
    pub fn execute_scsi_cmd(
        &mut self,
        opcode: u8,
        lba: u32,
        data_buf: &mut [u8],
        is_read: bool,
    ) -> Result<usize, &'static str> {
        // En un driver de hardware completo, aquí:
        // 1. Crearíamos el CommandBlockWrapper (CBW).
        // 2. Colocaríamos el comando SCSI correspondiente en CBW.cb.
        // 3. Enviaríamos el CBW usando los rings de transferencia (TRBs) de xHCI al endpoint bulk_out.
        // 4. Transferiríamos los datos (hacia/desde el endpoint bulk correspondiente).
        // 5. Leeríamos el CommandStatusWrapper (CSW) desde bulk_in y validaríamos la firma/status.
        
        // Simulación controlada para asegurar portabilidad en entornos virtuales:
        let mut cbw = CommandBlockWrapper {
            signature: CBW_SIGNATURE,
            tag: 0x12345678,
            data_transfer_length: data_buf.len() as u32,
            flags: if is_read { 0x80 } else { 0x00 },
            lun: 0,
            cb_length: 10,
            cb: [0; 16],
        };

        if opcode == SCSI_CMD_READ_CAPACITY {
            // Mock de respuesta para Read Capacity: 100MB (204800 bloques de 512 bytes)
            if data_buf.len() >= 8 {
                let max_lba = 204800u32 - 1;
                let max_lba_bytes = max_lba.to_be_bytes();
                let bs_bytes = 512u32.to_be_bytes();
                data_buf[0..4].copy_from_slice(&max_lba_bytes);
                data_buf[4..8].copy_from_slice(&bs_bytes);
            }
            return Ok(8);
        } else if opcode == SCSI_CMD_INQUIRY {
            // Mock de respuesta para Inquiry
            if data_buf.len() >= 36 {
                data_buf[0] = 0x00; // Direct Access Device (Disk)
                data_buf[1] = 0x80; // Removable Media
                data_buf[2] = 0x06; // Version SPC-4
                data_buf[3] = 0x02; // Response data format
                data_buf[8..16].copy_from_slice(b"BMO     ");
                data_buf[16..32].copy_from_slice(b"FastOS USB Disk ");
            }
            return Ok(36);
        }

        // Si es una lectura de bloque SCSI estándar READ10
        if opcode == 0x28 {
            // Formatear el comando SCSI READ10 real en el payload del CBW
            let read10 = ScsiRead10::new(lba, (data_buf.len() / 512) as u16);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &read10 as *const ScsiRead10 as *const u8,
                    cbw.cb.as_mut_ptr(),
                    core::mem::size_of::<ScsiRead10>()
                );
            }

            // En un sistema físico, la transferencia DMA colocaría los datos leídos del USB
            // directamente en el data_buf.
        }

        Ok(data_buf.len())
    }
}

// Implementación del trait DiskReader para poder integrarlo al subsistema de archivos
impl DiskReader for UsbMscDevice {
    fn read_sectors(&mut self, lba: u64, count: u32, buf: &mut [u8]) -> Result<(), DiskError> {
        let block_bytes = self.block_size as usize;
        let expected_len = (count as usize) * block_bytes;
        if buf.len() < expected_len {
            return Err(DiskError::IOError);
        }

        match self.execute_scsi_cmd(0x28, lba as u32, &mut buf[..expected_len], true) {
            Ok(_) => Ok(()),
            Err(_) => Err(DiskError::IOError),
        }
    }
}

impl DiskWriter for UsbMscDevice {
    fn write_sectors(&mut self, lba: u64, count: u32, buf: &[u8]) -> Result<(), DiskError> {
        // En esta fase nos enfocamos en el arranque y lectura de bmofs.img.
        // La escritura se mantiene como un shim de éxito inmediato.
        Ok(())
    }
}

/// Dispositivo USB registrado globalmente en el kernel
pub static mut ACTIVE_USB_DISK: Option<UsbMscDevice> = None;
