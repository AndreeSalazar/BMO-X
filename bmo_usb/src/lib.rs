#![no_std]

//! BMO-USB (Bare Metal Orchestrator USB Stack definitions)
//!
//! A modular, decoupled USB Host stack for FastOS. Contains specifications and structure
//! definitions for xHCI (USB 3.0 Host) registers, Command/Transfer Ring TRBs,
//! and SCSI/MSC (Mass Storage Class) Bulk-Only Transport formats.

// ── 1. xHCI MMIO Register Structures ─────────────────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CapRegisters {
    pub cap_length: u8,
    pub reserved: u8,
    pub hci_version: u16,
    pub hcs_params1: u32,
    pub hcs_params2: u32,
    pub hcs_params3: u32,
    pub hcc_params1: u32,
    pub dboff: u32,
    pub rtsoff: u32,
    pub hcc_params2: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OpRegisters {
    pub usb_cmd: u32,
    pub usb_sts: u32,
    pub page_size: u32,
    pub reserved1: [u32; 2],
    pub dnctrl: u32,
    pub crcr: u64,
    pub reserved2: [u32; 4],
    pub dcbaap: u64,
    pub config: u32,
}

// ── 2. TRB (Transfer Request Block) Formats ──────────────────────────

/// Estructura base de 16 bytes para cualquier TRB en xHCI.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Trb {
    pub parameter: u64,
    pub status: u32,
    pub control: u32,
}

impl Trb {
    pub const fn new(param: u64, status: u32, ctrl: u32) -> Self {
        Self {
            parameter: param,
            status,
            control: ctrl,
        }
    }

    pub fn get_type(&self) -> u8 {
        ((self.control >> 10) & 0x3F) as u8
    }

    pub fn cycle_bit(&self) -> bool {
        (self.control & 1) != 0
    }
}

// Tipos comunes de TRB
pub const TRB_TYPE_NORMAL:        u8 = 1;
pub const TRB_TYPE_SETUP_STAGE:   u8 = 2;
pub const TRB_TYPE_DATA_STAGE:    u8 = 3;
pub const TRB_TYPE_STATUS_STAGE:  u8 = 4;
pub const TRB_TYPE_LINK:          u8 = 8;
pub const TRB_TYPE_CMD_NOOP:      u8 = 23;

// ── 3. USB Mass Storage Class (MSC) - Bulk-Only Transport (BOT) ──────

/// Command Block Wrapper (CBW) — 31 bytes
/// Enviado por el Host al Device en el endpoint Bulk Out para indicar una operación SCSI.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CommandBlockWrapper {
    pub signature: u32,          // Debe ser 0x43425355 (USBC en ASCII little-endian)
    pub tag: u32,                // Identificador único de transacción
    pub data_transfer_length: u32,// Total de bytes que se transmitirán
    pub flags: u8,               // bit 7: Dirección (0=Out, 1=In)
    pub lun: u8,                 // Logical Unit Number (regularmente 0)
    pub cb_length: u8,           // Longitud del comando SCSI real (1..16)
    pub cb: [u8; 16],            // El bloque de comandos SCSI propiamente dicho
}

pub const CBW_SIGNATURE: u32 = 0x43425355;

/// Command Status Wrapper (CSW) — 13 bytes
/// Recibido por el Host desde el Device en el endpoint Bulk In al completar un comando.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CommandStatusWrapper {
    pub signature: u32,          // Debe ser 0x53425355 (USBS en ASCII)
    pub tag: u32,                // Debe coincidir con el tag enviado en el CBW
    pub data_residue: u32,       // Bytes no transferidos (diferencia con CBW)
    pub status: u8,              // 0=Success, 1=Command Failed, 2=Phase Error
}

pub const CSW_SIGNATURE: u32 = 0x53425355;

// ── 4. Comandos SCSI para Discos ─────────────────────────────────────

pub const SCSI_CMD_INQUIRY:      u8 = 0x12;
pub const SCSI_CMD_READ_CAPACITY: u8 = 0x25;
pub const SCSI_CMD_READ10:       u8 = 0x28;
pub const SCSI_CMD_WRITE10:      u8 = 0x2A;

/// Formato del comando SCSI READ (10)
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct ScsiRead10 {
    pub opcode: u8,              // 0x28
    pub reserved_and_lba_msb: u8,// bits 0..4 LBA msb
    pub lba: u32,                // Dirección LBA (Logical Block Address)
    pub group_number: u8,
    pub transfer_length: u16,    // Número de bloques a leer (en big-endian)
    pub control: u8,
}

impl ScsiRead10 {
    pub fn new(lba: u32, block_count: u16) -> Self {
        Self {
            opcode: SCSI_CMD_READ10,
            reserved_and_lba_msb: 0,
            lba: lba.to_be(),
            group_number: 0,
            transfer_length: block_count.to_be(),
            control: 0,
        }
    }
}
