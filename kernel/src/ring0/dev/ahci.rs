//! AHCI/SATA Storage Driver (Ring 0 HAL).
//!
//! Manages Serial ATA controllers via the AHCI (Advanced Host Controller
//! Interface) specification. Detects ports, reads HBA capabilities,
//! issues commands via command list + FIS + PRDT.
//!
//! Hardware registers (HBA memory registers):
//!   - CAP: Capabilities (port count, command slots, speed)
//!   - GHC: Global HBA Control (reset, interrupt enable)
//!   - IS: Interrupt Status
//!   - PI: Ports Implemented
//!   - VS: Version
//!   - CAP2: Extended capabilities
//!
//! Port registers (per-port, 0x100-byte stride):
//!   - CLB/U: Command List Base Address
//!   - FB/U: FIS Base Address
//!   - IS: Port Interrupt Status
//!   - CMD: Command and Status
//!   - TFD: Task File Data
//!   - SIG: Signature
//!   - SSTS: Serial ATA Status
//!   - SERR: Serial ATA Error
//!   - CI: Command Issue

use super::storage::StorageDevice;
use super::storage::StorageType;

/// AHCI HBA register offsets (memory-mapped).
const HBA_CAP: usize = 0x00;
const HBA_GHC: usize = 0x04;
const HBA_IS: usize = 0x08;
const HBA_PI: usize = 0x0C;
const HBA_VS: usize = 0x10;
const HBA_CAP2: usize = 0x24;

/// Port register stride (0x100 bytes per port).
const PORT_STRIDE: usize = 0x100;

/// Port register offsets (within port block).
const PORT_CLB: usize = 0x00;
const PORT_CLBU: usize = 0x04;
const PORT_FB: usize = 0x08;
const PORT_FBU: usize = 0x0C;
const PORT_IS: usize = 0x10;
const PORT_CMD: usize = 0x18;
const PORT_TFD: usize = 0x20;
const PORT_SIG: usize = 0x24;
const PORT_SSTS: usize = 0x28;
const PORT_SERR: usize = 0x30;
const PORT_CI: usize = 0x38;

/// HBA GHC bits.
const GHC_HR: u32 = 1 << 0;  // HBA Reset
const GHC_IE: u32 = 1 << 1;  // Interrupt Enable

/// Port CMD bits.
const CMD_ST: u32 = 1 << 0;   // Start
const CMD_FRE: u32 = 1 << 4;  // FIS Receive Enable
const CMD_FR: u32 = 1 << 14;  // FIS Receive Running
const CMD_CR: u32 = 1 << 15;  // Command List Running

/// Port SSTS bits.
const SSTS_DET: u32 = 0x0F;   // Device Detection
const SSTS_SPD: u32 = 0xF0;   // Interface Speed
const SSTS_IPM: u32 = 0xF00;  // Interface Power Management

/// AHCI port state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    Empty,
    Present,
    Active,
    Error,
    Unknown,
}

/// AHCI port information.
#[derive(Debug, Clone, Copy)]
pub struct AhciPort {
    pub port_number: u8,
    pub state: PortState,
    pub signature: u32,
    pub command_list_phys: u64,
    pub fis_phys: u64,
    pub sectors_total: u64,
}

/// AHCI controller state.
#[derive(Debug)]
pub struct AhciController {
    pub mmio_base: u64,
    pub port_count: u8,
    pub ports_implemented: u32,
    pub ports: [AhciPort; 32],
    pub irq: u8,
}

static mut CONTROLLER: Option<AhciController> = None;

/// Read a 32-bit AHCI HBA register.
unsafe fn hba_read(mmio: u64, offset: usize) -> u32 {
    let ptr = (mmio + offset as u64) as *const u32;
    core::ptr::read_volatile(ptr)
}

/// Write a 32-bit AHCI HBA register.
unsafe fn hba_write(mmio: u64, offset: usize, val: u32) {
    let ptr = (mmio + offset as u64) as *mut u32;
    core::ptr::write_volatile(ptr, val);
}

/// Read a 32-bit port register.
unsafe fn port_read(mmio: u64, port: u8, offset: usize) -> u32 {
    let base = mmio + 0x100 + (port as u64) * (PORT_STRIDE as u64);
    let ptr = (base + offset as u64) as *const u32;
    core::ptr::read_volatile(ptr)
}

/// Write a 32-bit port register.
unsafe fn port_write(mmio: u64, port: u8, offset: usize, val: u32) {
    let base = mmio + 0x100 + (port as u64) * (PORT_STRIDE as u64);
    let ptr = (base + offset as u64) as *mut u32;
    core::ptr::write_volatile(ptr, val);
}

/// Probe an AHCI controller at the given MMIO base address.
///
/// Steps:
///   1. Read HBA capabilities
///   2. Reset HBA (GHC.HR)
///   3. Enable interrupts (GHC.IE)
///   4. Enumerate implemented ports
///   5. Read port signatures and detect devices
pub unsafe fn probe(mmio_base: u64, irq: u8) {
    crate::dev::console::serial_write("[ahci] probing MMIO=0x");
    crate::dev::console::serial_write_u64(mmio_base, 16);
    crate::dev::console::serial_write("\n");

    // Read capabilities
    let cap = hba_read(mmio_base, HBA_CAP);
    let port_count = ((cap >> 20) & 0x1F) as u8 + 1;
    let slots = ((cap >> 8) & 0x1F) as u8 + 1;

    crate::dev::console::serial_write("[ahci] CAP: ports=");
    crate::dev::console::serial_write_u64(port_count as u64, 10);
    crate::dev::console::serial_write(" slots=");
    crate::dev::console::serial_write_u64(slots as u64, 10);
    crate::dev::console::serial_write("\n");

    // HBA Reset
    hba_write(mmio_base, HBA_GHC, GHC_HR);
    let mut timeout = 100_000u32;
    while hba_read(mmio_base, HBA_GHC) & GHC_HR != 0 && timeout > 0 {
        timeout -= 1;
    }
    if timeout == 0 {
        crate::dev::console::serial_write("[ahci] ERROR: HBA reset timeout\n");
        return;
    }

    // Enable interrupts
    hba_write(mmio_base, HBA_GHC, GHC_IE);

    // Read ports implemented
    let pi = hba_read(mmio_base, HBA_PI);

    // Init controller struct
    let mut ctrl = AhciController {
        mmio_base,
        port_count,
        ports_implemented: pi,
        ports: [AhciPort {
            port_number: 0,
            state: PortState::Empty,
            signature: 0,
            command_list_phys: 0,
            fis_phys: 0,
            sectors_total: 0,
        }; 32],
        irq,
    };

    // Enumerate ports
    for i in 0..port_count.min(32) {
        if pi & (1 << i) == 0 { continue; }

        let ssts = port_read(mmio_base, i, PORT_SSTS);
        let det = ssts & SSTS_DET;

        let state = match det {
            0x01 => PortState::Present,
            0x03 => PortState::Active,
            _ => PortState::Empty,
        };

        let sig = port_read(mmio_base, i, PORT_SIG);

        ctrl.ports[i as usize] = AhciPort {
            port_number: i,
            state,
            signature: sig,
            command_list_phys: 0,
            fis_phys: 0,
            sectors_total: 0,
        };

        if state == PortState::Active {
            crate::dev::console::serial_write("[ahci] port ");
            crate::dev::console::serial_write_u64(i as u64, 10);
            crate::dev::console::serial_write(" active, sig=0x");
            crate::dev::console::serial_write_u64(sig as u64, 16);
            crate::dev::console::serial_write("\n");
        }
    }

    CONTROLLER = Some(ctrl);

    crate::dev::console::serial_write("[ahci] probe complete\n");
}

/// Get reference to the AHCI controller (if initialized).
pub fn controller() -> Option<&'static AhciController> {
    unsafe { CONTROLLER.as_ref() }
}

/// Check if AHCI controller is initialized.
pub fn is_initialized() -> bool {
    unsafe { CONTROLLER.is_some() }
}
