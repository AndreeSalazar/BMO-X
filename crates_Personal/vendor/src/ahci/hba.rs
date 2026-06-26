use core::ptr::{read_volatile, write_volatile};

/// AHCI Host Bus Adapter (HBA) register definitions.
/// Based on AHCI spec 1.4.

/// HBA memory-mapped registers (at ABAR).
#[repr(C)]
pub struct HbaRegisters {
    pub cap: u32,        // 0x00: Host Capabilities
    pub ghc: u32,        // 0x04: Global Host Control
    pub is: u32,         // 0x08: Interrupt Status
    pub pi: u32,         // 0x0C: Ports Implemented
    pub vs: u32,         // 0x10: Version
    pub ccc_ctl: u32,    // 0x14: Command Completion Coalescing Control
    pub ccc_pts: u32,    // 0x18: Command Completion Coalescing Ports
    pub em_loc: u32,     // 0x1C: Enclosure Management Location
    pub em_ctl: u32,     // 0x20: Enclosure Management Control
    pub cap2: u32,       // 0x24: Host Capabilities Extended
    pub bohc: u32,       // 0x28: BIOS/OS Handoff Control and Status
    // 0x2C-0x9F: reserved
    pub nvmhci: [u32; 4], // 0xA0: NVMHCI registers (if present)
    // 0xB0-0xFF: reserved
    pub vendor: [u32; 16], // 0xA0-0xDF: vendor specific
}

/// AHCI port registers (at ABAR + 0x100 + port*0x80).
#[repr(C)]
pub struct PortRegisters {
    pub clb: u32,        // 0x00: Command List Base Address (low)
    pub clbu: u32,       // 0x04: Command List Base Address (high)
    pub fb: u32,         // 0x08: FIS Base Address (low)
    pub fbu: u32,        // 0x0C: FIS Base Address (high)
    pub is: u32,         // 0x10: Interrupt Status
    pub ie: u32,         // 0x14: Interrupt Enable
    pub cmd: u32,        // 0x18: Command and Status
    pub _r0: u32,        // 0x1C: reserved
    pub tfd: u32,        // 0x20: Task File Data
    pub sig: u32,        // 0x24: Signature
    pub ssts: u32,       // 0x28: SATA Status
    pub sctl: u32,       // 0x2C: SATA Control
    pub serr: u32,       // 0x30: SATA Error
    pub sact: u32,       // 0x34: SATA Active
    pub ci: u32,         // 0x38: Command Issue
    pub sntf: u32,       // 0x3C: SATA Notification
    pub fbs: u32,        // 0x40: FIS-based Switching Control
    pub devslp: u32,     // 0x44: Device Sleep
    pub _r1: [u32; 10],  // 0x48-0x6F: reserved
    pub vendor: [u32; 4], // 0x70-0x7C: vendor specific
}

/// AHCI command header (in command list).
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct CmdHeader {
    pub opts: u32,       // DW0: command options
    pub status: u32,     // DW1: PRDT byte count (write)
    pub tbl_addr_lo: u32, // DW2: command table base (low)
    pub tbl_addr_hi: u32, // DW3: command table base (high)
    pub reserved: [u32; 4], // DW4-7: reserved
}

/// AHCI command table (for FIS + SG list).
#[repr(C)]
pub struct CmdTable {
    pub cfis: [u8; 64],   // Command FIS (up to 64 bytes)
    pub acmd: [u8; 16],   // ATAPI command (16 bytes)
    pub reserved: [u8; 48], // reserved
    pub sg: [SgEntry; 56], // Scatter-gather entries (PRDT)
}

/// Scatter-gather entry (PRDT).
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct SgEntry {
    pub addr_lo: u32,
    pub addr_hi: u32,
    pub reserved: u32,
    pub flags_size: u32, // bits 0-21: byte count - 1, bit 31: interrupt on completion
}

/// SATA FIS Register - Host to Device (20 bytes).
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct FisH2D {
    pub fis_type: u8,     // 0x27
    pub pm_port: u8,      // port multiplier, bit 7 = command
    pub command: u8,
    pub features_lo: u8,
    pub lba_lo: u8,
    pub lba_mid: u8,
    pub lba_hi: u8,
    pub device: u8,
    pub lba_lo_exp: u8,
    pub lba_mid_exp: u8,
    pub lba_hi_exp: u8,
    pub features_hi: u8,
    pub sector_count_lo: u8,
    pub sector_count_hi: u8,
    pub _r0: u8,
    pub control: u8,
    pub _r1: [u8; 4],
}

impl FisH2D {
    pub fn new_command(cmd: u8, lba: u64, count: u16, lba48: bool) -> Self {
        let mut fis = FisH2D {
            fis_type: 0x27,
            pm_port: 0x80, // command bit
            command: cmd,
            device: if lba48 { 0x40 } else { 0x00 },
            ..Default::default()
        };
        if lba48 {
            fis.lba_lo = lba as u8;
            fis.lba_mid = (lba >> 8) as u8;
            fis.lba_hi = (lba >> 16) as u8;
            fis.lba_lo_exp = (lba >> 24) as u8;
            fis.lba_mid_exp = (lba >> 32) as u8;
            fis.lba_hi_exp = (lba >> 40) as u8;
            fis.sector_count_lo = count as u8;
            fis.sector_count_hi = (count >> 8) as u8;
        } else {
            fis.lba_lo = lba as u8;
            fis.lba_mid = (lba >> 8) as u8;
            fis.lba_hi = (lba >> 16) as u8;
            fis.device |= ((lba >> 24) as u8) & 0x0F;
            fis.sector_count_lo = count as u8;
        }
        fis
    }
}

// ── MMIO helpers ──────────────────────────────────────────────

pub unsafe fn mmio_read32(base: usize, offset: u32) -> u32 {
    read_volatile((base + offset as usize) as *const u32)
}

pub unsafe fn mmio_write32(base: usize, offset: u32, val: u32) {
    write_volatile((base + offset as usize) as *mut u32, val);
}

/// Timeout helper: polls a condition with TSC-based timeout.
pub fn wait_until<F: Fn() -> bool>(timeout_ms: u64, mut f: F) -> bool {
    // Use a simple loop counter as timeout approximation
    // (real implementation would use RDTSC)
    let iterations = timeout_ms * 10_000;
    for _ in 0..iterations {
        if f() { return true; }
        core::hint::spin_loop();
    }
    false
}

// AHCI commands
pub const ATA_CMD_IDENTIFY: u8 = 0xEC;
pub const ATA_CMD_IDENTIFY_EXT: u8 = 0x27;
pub const ATA_CMD_READ_DMA_EXT: u8 = 0x25;
pub const ATA_CMD_WRITE_DMA_EXT: u8 = 0x35;
pub const ATA_CMD_READ_EXT: u8 = 0x24;
pub const ATA_CMD_WRITE_EXT: u8 = 0x34;

// HBA register bit positions
pub const GHC_AE: u32 = 1 << 31;   // AHCI Enable
pub const GHC_HR: u32 = 1 << 0;    // HBA Reset
pub const GHC_IE: u32 = 1 << 1;    // Interrupt Enable

// Port command bits
pub const PORT_CMD_ST: u32 = 1 << 0;   // Start
pub const PORT_CMD_FRE: u32 = 1 << 4;  // FIS Receive Enable
pub const PORT_CMD_CLO: u32 = 1 << 3;  // Command List Overrun
pub const PORT_CMD_SUD: u32 = 1 << 1;  // Spin-Up Device
pub const PORT_CMD_POD: u32 = 1 << 2;  // Power On Device
pub const PORT_CMD_ICC_ACTIVE: u32 = 1 << 28; // Interface Communication Control

// SATA Status
pub const SSTS_DET_PRESENT: u32 = 0x03; // Device present and communication established

// Task File Data bits
pub const TFD_BSY: u32 = 0x80;
pub const TFD_DRQ: u32 = 0x08;
pub const TFD_ERR: u32 = 0x01;
