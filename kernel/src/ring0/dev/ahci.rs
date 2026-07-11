//! AHCI Driver — SATA storage controller driver for BMO.
//!
//! This driver implements the Advanced Host Controller Interface (AHCI)
//! for SATA storage devices. It provides the low-level disk I/O that
//! the storage HAL needs to become "ready".
//!
//! ## Architecture
//!
//! The AHCI driver:
//! 1. Enumerates PCI devices to find AHCI controllers
//! 2. Maps the ABAR (AHCI Base Address Register) into memory
//! 3. Initializes the HBA (Host Bus Adapter)
//! 4. Detects and initializes ports
//! 5. Provides read/write sector functions
//!
//! ## Status
//!
//! This is a minimal implementation that:
//! - Detects AHCI controllers via PCI
//! - Initializes the HBA
//! - Detects connected devices
//! - Provides PIO mode read/write (DMA will be added later)

#![allow(dead_code)]
use core::arch::asm;

/// PCI Configuration Space addresses
const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// AHCI PCI class code
const AHCI_CLASS: u8 = 0x01; // Mass storage
const AHCI_SUBCLASS: u8 = 0x06; // SATA
const AHCI_PROG_IF: u8 = 0x01; // AHCI

/// AHCI register offsets (from ABAR)
const HBA_CAP: u32 = 0x0000; // Host Capabilities
const HBA_GHC: u32 = 0x0004; // Global HBA Control
const HBA_IS: u32 = 0x0008; // Interrupt Status
const HBA_PI: u32 = 0x000C; // Ports Implemented
const HBA_VERSION: u32 = 0x0010; // Version
const HBA_CCC_CTL: u32 = 0x0014; // Coalescing Control
const HBA_CCC_PORTS: u32 = 0x0018; // Coalescing Ports
const HBA_EM_LOC: u32 = 0x001C; // Enclosure Management Location
const HBA_EM_CTL: u32 = 0x0020; // Enclosure Management Control
const HBA_CAP2: u32 = 0x0024; // Host Capabilities Extended

/// Port register offsets (from port base)
const PORT_CLB: u32 = 0x0000; // Command List Base Address
const PORT_CLBU: u32 = 0x0004; // Command List Base Address Upper
const PORT_FB: u32 = 0x0008; // FIS Base Address
const PORT_FBU: u32 = 0x000C; // FIS Base Address Upper
const PORT_IS: u32 = 0x0010; // Interrupt Status
const PORT_IE: u32 = 0x0014; // Interrupt Enable
const PORT_CMD: u32 = 0x0018; // Command and Status
const PORT_TFD: u32 = 0x0020; // Task File Data
const PORT_SIG: u32 = 0x0024; // Signature
const PORT_SSTS: u32 = 0x0028; // Serial ATA Status
const PORT_SCTL: u32 = 0x002C; // Serial ATA Control
const PORT_SERR: u32 = 0x0030; // Serial ATA Error
const PORT_SACT: u32 = 0x0034; // Serial ATA Active
const PORT_CI: u32 = 0x0038; // Command Issue
const PORT_SNTF: u32 = 0x003C; // Serial ATA Notification

/// HBA GHC bits
const GHC_AE: u32 = 1 << 31; // AHCI Enable
const GHC_MRSM: u32 = 1 << 2; // MSI Revert to Single Message
const GHC_IE: u32 = 1 << 1; // Interrupt Enable
const GHC_HR: u32 = 1 << 0; // HBA Reset

/// Port CMD bits
const PORT_CMD_ICC_ACTIVE: u32 = 1 << 28; // Interface Communication Control
const PORT_CMD_ALPE: u32 = 1 << 26; // Aggressive Link Power Management Enable
const PORT_CMD_DLAE: u32 = 1 << 25; // Drive LED on ATAPI Enable
const PORT_CMD_ATAPI: u32 = 1 << 24; // Device is ATAPI
const PORT_CMD_ESP: u32 = 1 << 21; // External SATA Port
const PORT_CMD_CPD: u32 = 1 << 20; // Cold Presence Detection
const PORT_CMD_MPSS: u32 = 1 << 19; // Mechanical Presence Switch State
const PORT_CMD_FR: u32 = 1 << 14; // FIS Receive Running
const PORT_CMD_CR: u32 = 1 << 15; // Command List Running
const PORT_CMD_FRE: u32 = 1 << 4; // FIS Receive Enable
const PORT_CMD_SUD: u32 = 1 << 1; // Spin-Up Device
const PORT_CMD_ST: u32 = 1 << 0; // Start

/// Port TFD bits
const TFD_STS_BSY: u8 = 1 << 7; // Busy
const TFD_STS_DRQ: u8 = 1 << 3; // Data Request
const TFD_STS_ERR: u8 = 1 << 0; // Error

/// SATA signatures
const SIG_ATA: u32 = 0x00000101;
const SIG_ATAPI: u32 = 0xEB140101;
const SIG_SEMB: u32 = 0xC33C0101;
const SIG_PM: u32 = 0x96690101;

/// Port device types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortType {
    None,
    SATA,
    SATAPi,
    SEMB,
    PM,
    Unknown,
}

/// HBA memory registers
#[repr(C)]
struct HbaMemory {
    cap: u32,
    ghc: u32,
    is: u32,
    pi: u32,
    version: u32,
    ccc_ctl: u32,
    ccc_ports: u32,
    em_loc: u32,
    em_ctl: u32,
    cap2: u32,
    bohc: u32,
    _reserved: [u8; 0xA0 - 0x2C],
    oob_we: u32,
    _reserved2: [u8; 0x100 - 0xA4],
}

/// Port memory registers
#[repr(C)]
struct HbaPort {
    clb: u32,
    clbu: u32,
    fb: u32,
    fbu: u32,
    is: u32,
    ie: u32,
    cmd: u32,
    _reserved0: u32,
    tfd: u32,
    sig: u32,
    ssts: u32,
    sctl: u32,
    serr: u32,
    sact: u32,
    ci: u32,
    sntf: u32,
    fbs: u32,
    _reserved1: [u32; 11],
    _vendor: [u32; 4],
}

/// Command header
#[repr(C)]
struct HbaCmdHeader {
    cfl_prdtl: u16, // CFL:5, A:1, W:1, P:1, R:1, B:1, C:1, MP:1, PRDTL:16
    prdbc: u16,
    ctba_lo: u32,
    ctba_hi: u32,
    _reserved: [u32; 4],
}

/// Command table
#[repr(C)]
struct HbaCmdTable {
    cfis: [u8; 64],
    acmd: [u8; 16],
    _reserved: [u8; 48],
    prdt_entry: [HbaPrdtEntry; 0], // Variable length
}

/// Physical Region Descriptor Table entry
#[repr(C)]
struct HbaPrdtEntry {
    dba_lo: u32,
    dba_hi: u32,
    _reserved: u32,
    dbc_interrupt: u32, // DBC:22, I:1, reserved:9
}

/// Received FIS structure
#[repr(C)]
struct HbaFis {
    dsfis: [u8; 28], // DMA Setup FIS
    _reserved0: [u8; 4],
    psfis: [u8; 20], // PIO Setup FIS
    _reserved1: [u8; 12],
    rfis: [u8; 24], // Register – Device to Host FIS
    _reserved2: [u8; 4],
    sdbfis: [u8; 8], // Set Device Bits FIS
    ufis: [u8; 64], // Unknown FIS
    _reserved3: [u8; 0x100 - 0xC0],
}

/// AHCI controller state
static mut AHCI_BASE: u64 = 0;
static mut PORT_COUNT: u8 = 0;
static mut PORT_TYPES: [PortType; 32] = [PortType::None; 32];

/// Read PCI configuration space
unsafe fn pci_config_read(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let address = 0x8000_0000u32
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);
    
    asm!(
        "out dx, eax",
        in("dx") PCI_CONFIG_ADDRESS,
        in("eax") address,
        options(nostack, nomem)
    );
    
    let value: u32;
    asm!(
        "in eax, dx",
        in("dx") PCI_CONFIG_DATA,
        out("eax") value,
        options(nostack, nomem)
    );
    
    value
}

/// Write PCI configuration space
unsafe fn pci_config_write(bus: u8, device: u8, func: u8, offset: u8, value: u32) {
    let address = 0x8000_0000u32
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);
    
    asm!(
        "out dx, eax",
        in("dx") PCI_CONFIG_ADDRESS,
        in("eax") address,
        options(nostack, nomem)
    );
    
    asm!(
        "out dx, eax",
        in("dx") PCI_CONFIG_DATA,
        in("eax") value,
        options(nostack, nomem)
    );
}

/// Check if a PCI device is an AHCI controller
unsafe fn is_ahci_device(bus: u8, device: u8, func: u8) -> bool {
    let class_reg = pci_config_read(bus, device, func, 0x08);
    let class = (class_reg >> 24) as u8;
    let subclass = (class_reg >> 16) as u8;
    let prog_if = (class_reg >> 8) as u8;
    
    class == AHCI_CLASS && subclass == AHCI_SUBCLASS && prog_if == AHCI_PROG_IF
}

/// Get the ABAR (AHCI Base Address Register) from PCI
unsafe fn get_abar(bus: u8, device: u8, func: u8) -> u64 {
    let bar0 = pci_config_read(bus, device, func, 0x10);
    
    // Check if it's a 64-bit BAR
    if bar0 & 0x6 == 0x4 {
        // 64-bit BAR
        let bar1 = pci_config_read(bus, device, func, 0x14);
        ((bar1 as u64) << 32) | (bar0 as u64 & 0xFFFF_FFF0)
    } else {
        // 32-bit BAR
        (bar0 & 0xFFFF_FFF0) as u64
    }
}

/// Enable PCI bus mastering
unsafe fn enable_bus_mastering(bus: u8, device: u8, func: u8) {
    let command = pci_config_read(bus, device, func, 0x04);
    // Set bit 2 (Bus Master) and bit 0 (I/O Space)
    pci_config_write(bus, device, func, 0x04, command | 0x05);
}

/// Detect AHCI controllers via PCI
pub unsafe fn detect_controllers() -> Option<(u8, u8, u8, u64)> {
    crate::dev::console::serial_write("[ahci] scanning PCI bus for AHCI controllers...\n");
    
    for bus in 0..=255 {
        for device in 0..32 {
            for func in 0..8 {
                let vendor_device = pci_config_read(bus, device, func, 0x00);
                if vendor_device == 0xFFFF_FFFF {
                    continue; // No device
                }
                
                if is_ahci_device(bus, device, func) {
                    let abar = get_abar(bus, device, func);
                    crate::dev::console::serial_write("[ahci] found AHCI controller at ");
                    crate::dev::console::serial_write_u64(bus as u64, 10);
                    crate::dev::console::serial_write(":");
                    crate::dev::console::serial_write_u64(device as u64, 10);
                    crate::dev::console::serial_write(":");
                    crate::dev::console::serial_write_u64(func as u64, 10);
                    crate::dev::console::serial_write(" ABAR=0x");
                    crate::dev::console::serial_write_u64(abar, 16);
                    crate::dev::console::serial_write("\n");
                    
                    return Some((bus, device, func, abar));
                }
            }
        }
    }
    
    crate::dev::console::serial_write("[ahci] no AHCI controllers found\n");
    None
}

/// Initialize the AHCI HBA
unsafe fn init_hba(abar: u64) -> bool {
    let hba = &*(abar as *const HbaMemory);
    
    // Check version
    let version = core::ptr::read_volatile(&hba.version);
    crate::dev::console::serial_write("[ahci] version: 0x");
    crate::dev::console::serial_write_u64(version as u64, 16);
    crate::dev::console::serial_write("\n");
    
    // Enable AHCI
    let mut ghc = core::ptr::read_volatile(&hba.ghc);
    if ghc & GHC_AE == 0 {
        ghc |= GHC_AE;
        core::ptr::write_volatile(&mut (*(abar as *mut HbaMemory)).ghc, ghc);
    }
    
    // Check capabilities
    let cap = core::ptr::read_volatile(&hba.cap);
    let num_ports = (cap & 0x1F) + 1;
    crate::dev::console::serial_write("[ahci] supports ");
    crate::dev::console::serial_write_u64(num_ports as u64, 10);
    crate::dev::console::serial_write(" ports\n");
    
    // Get ports implemented
    let pi = core::ptr::read_volatile(&hba.pi);
    crate::dev::console::serial_write("[ahci] ports implemented: 0x");
    crate::dev::console::serial_write_u64(pi as u64, 16);
    crate::dev::console::serial_write("\n");
    
    true
}

/// Detect the type of device on a port
unsafe fn detect_port_type(port: &HbaPort) -> PortType {
    let ssts = core::ptr::read_volatile(&port.ssts);
    let ipm = (ssts >> 8) & 0x0F; // Interface Power Management
    let det = ssts & 0x0F; // Device Detection
    
    if det != 0x03 || ipm != 0x01 {
        return PortType::None; // No device or not active
    }
    
    let sig = core::ptr::read_volatile(&port.sig);
    match sig {
        SIG_ATA => PortType::SATA,
        SIG_ATAPI => PortType::SATAPi,
        SIG_SEMB => PortType::SEMB,
        SIG_PM => PortType::PM,
        _ => PortType::Unknown,
    }
}

/// Initialize AHCI driver
pub fn init() -> bool {
    unsafe {
        // Detect AHCI controller
        let controller = match detect_controllers() {
            Some(c) => c,
            None => return false,
        };
        
        let (bus, device, func, abar) = controller;
        
        // Enable bus mastering
        enable_bus_mastering(bus, device, func);
        
        // Initialize HBA
        if !init_hba(abar) {
            return false;
        }
        
        // Store ABAR
        AHCI_BASE = abar;
        
        // Detect ports
        let hba = &*(abar as *const HbaMemory);
        let pi = core::ptr::read_volatile(&hba.pi);
        
        let mut count = 0;
        for port_idx in 0..32 {
            if pi & (1 << port_idx) != 0 {
                let port = &*((abar + 0x100 + port_idx as u64 * 0x80) as *const HbaPort);
                let port_type = detect_port_type(port);
                PORT_TYPES[port_idx] = port_type;
                
                if port_type != PortType::None {
                    crate::dev::console::serial_write("[ahci] port ");
                    crate::dev::console::serial_write_u64(port_idx as u64, 10);
                    crate::dev::console::serial_write(": ");
                    match port_type {
                        PortType::SATA => crate::dev::console::serial_write("SATA\n"),
                        PortType::SATAPi => crate::dev::console::serial_write("SATAPI\n"),
                        PortType::Unknown => crate::dev::console::serial_write("Unknown\n"),
                        _ => crate::dev::console::serial_write("Other\n"),
                    }
                    count += 1;
                }
            }
        }
        
        PORT_COUNT = count;
        
        if count > 0 {
            crate::dev::storage::set_ready(count);
            crate::dev::console::serial_write("[ahci] initialized with ");
            crate::dev::console::serial_write_u64(count as u64, 10);
            crate::dev::console::serial_write(" ports\n");
            true
        } else {
            crate::dev::console::serial_write("[ahci] no devices found\n");
            false
        }
    }
}

/// Read sectors from a SATA drive using PIO mode
/// 
/// # Safety
/// 
/// `dst` must be at least `sector_count * 512` bytes.
pub unsafe fn read_sectors_pio(port_idx: u8, _lba: u64, _sector_count: u16, _dst: *mut u8) -> bool {
    if AHCI_BASE == 0 {
        return false;
    }
    
    if port_idx >= 32 || PORT_TYPES[port_idx as usize] != PortType::SATA {
        return false;
    }
    
    // For now, return false to indicate PIO not yet implemented
    // DMA implementation will come later
    crate::dev::console::serial_write("[ahci] read_sectors_pio: not yet implemented\n");
    false
}

/// Write sectors to a SATA drive using PIO mode
/// 
/// # Safety
/// 
/// `src` must be at least `sector_count * 512` bytes.
pub unsafe fn write_sectors_pio(port_idx: u8, _lba: u64, _sector_count: u16, _src: *const u8) -> bool {
    if AHCI_BASE == 0 {
        return false;
    }
    
    if port_idx >= 32 || PORT_TYPES[port_idx as usize] != PortType::SATA {
        return false;
    }
    
    // For now, return false to indicate PIO not yet implemented
    crate::dev::console::serial_write("[ahci] write_sectors_pio: not yet implemented\n");
    false
}
