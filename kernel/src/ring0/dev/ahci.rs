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

// ── DMA Read ───────────────────────────────────────────────────────

/// FIS types.
const FIS_TYPE_REG_H2D: u8 = 0x27; // Register Host-to-Device

/// ATA commands.
const ATA_CMD_IDENTIFY: u8 = 0xEC;
const ATA_CMD_READ_DMA_EX: u8 = 0x25;

/// Initialize DMA structures for a port (command list + FIS).
/// Must be called with port CMD.ST=0 and CMD.FRE=0.
pub unsafe fn init_port_dma(port_idx: u8) -> bool {
    let ctrl = match CONTROLLER.as_mut() {
        Some(c) => c,
        None => return false,
    };
    let port = &mut ctrl.ports[port_idx as usize];
    if port.state != PortState::Active { return false; }

    let mmio = ctrl.mmio_base;

    // Allocate 1-page aligned command list (1 KB, 32 slots × 32 bytes)
    let cl_phys = match crate::mm::phys::alloc_pages_contiguous(1) {
        Some(p) => p,
        None => return false,
    };
    let cl_virt = crate::mm::vmm::phys_to_virt(cl_phys) as *mut u8;
    core::ptr::write_bytes(cl_virt, 0, 4096);
    port.command_list_phys = cl_phys;

    // Allocate 1-page FIS receive area
    let fis_phys = match crate::mm::phys::alloc_pages_contiguous(1) {
        Some(p) => p,
        None => return false,
    };
    let fis_virt = crate::mm::vmm::phys_to_virt(fis_phys) as *mut u8;
    core::ptr::write_bytes(fis_virt, 0, 4096);
    port.fis_phys = fis_phys;

    // Allocate command table for slot 0 (128 bytes)
    let ct_phys = match crate::mm::phys::alloc_pages_contiguous(1) {
        Some(p) => p,
        None => return false,
    };
    let ct_virt = crate::mm::vmm::phys_to_virt(ct_phys) as *mut u8;
    core::ptr::write_bytes(ct_virt, 0, 4096);

    // Write command list header (slot 0) — points to command table
    // Core::ptr::write_bytes already zeroed everything
    // Command header: [31:0]=CTBA (command table phys, 128-byte aligned)
    // [15:0]=CFL (command FIS length in DWORDS, 5 for H2D)
    // [21:16]=A (ATAPI), [28]=W (write), [31]=P (prefetch)
    let cl_hdr = cl_virt as *mut u32;
    cl_hdr.write_volatile(ct_phys as u32);           // CTBA low
    cl_hdr.add(1).write_volatile(0);                  // CTBA high
    cl_hdr.add(2).write_volatile(0);                  // reserved
    cl_hdr.add(3).write_volatile(0);                  // PRDBC = 0

    // Build command FIS in command table
    let ct = ct_virt as *mut u32;
    ct.write_volatile(5_u32 | ((FIS_TYPE_REG_H2D as u32) << 0)); // FIS header: 5 dwords + type

    // Program command list + FIS base in port registers
    port_write(mmio, port_idx, PORT_CLB, cl_phys as u32);
    port_write(mmio, port_idx, PORT_CLBU, (cl_phys >> 32) as u32);
    port_write(mmio, port_idx, PORT_FB, fis_phys as u32);
    port_write(mmio, port_idx, PORT_FBU, (fis_phys >> 32) as u32);

    // Start port: set FRE + ST, wait for CR + FR
    let mut cmd = port_read(mmio, port_idx, PORT_CMD);
    while cmd & CMD_CR != 0 || cmd & CMD_FR != 0 {
        port_write(mmio, port_idx, PORT_CMD, cmd & !(CMD_ST | CMD_FRE));
        core::hint::spin_loop();
        cmd = port_read(mmio, port_idx, PORT_CMD);
    }
    port_write(mmio, port_idx, PORT_CMD, CMD_FRE | CMD_ST);

    // Wait for FRE+ST to be active
    for _ in 0..1000 {
        cmd = port_read(mmio, port_idx, PORT_CMD);
        if (cmd & CMD_FR) != 0 && (cmd & CMD_CR) != 0 { break; }
        core::hint::spin_loop();
    }

    true
}

/// Read `sector_count` sectors starting at LBA `lba` into `buf`.
/// Returns number of sectors actually read, or 0 on error.
pub unsafe fn read_sectors(port_idx: u8, lba: u64, sector_count: u16, buf: *mut u8) -> u16 {
    let ctrl = match CONTROLLER.as_ref() {
        Some(c) => c,
        None => return 0,
    };
    let port = &ctrl.ports[port_idx as usize];
    let mmio = ctrl.mmio_base;

    if port.command_list_phys == 0 { return 0; }

    // Build the command FIS: READ DMA EXT
    let cl_virt = crate::mm::vmm::phys_to_virt(port.command_list_phys) as *mut u8;
    // Command header CTBA already set during init
    let cl_hdr = cl_virt as *mut u32;
    let ct_phys = cl_hdr.read_volatile() as u64;
    let ct_virt = crate::mm::vmm::phys_to_virt(ct_phys) as *mut u8;

    // Command FIS (H2D): starts at ct_virt + 0
    let fis = ct_virt;
    fis.write_volatile(FIS_TYPE_REG_H2D);       // FIS type
    fis.add(1).write_volatile(0x80);             // C=1 (command)
    fis.add(2).write_volatile(ATA_CMD_READ_DMA_EX);
    fis.add(3).write_volatile(0);                // Features low (0)
    // LBA (48-bit, bytes 4-9)
    let lb = lba.to_le_bytes();
    fis.add(4).write_volatile(lb[0]);  // LBA low
    fis.add(5).write_volatile(lb[1]);  // LBA mid
    fis.add(6).write_volatile(lb[2]);  // LBA high
    fis.add(7).write_volatile(0x40);   // Device (LBA mode)
    fis.add(8).write_volatile(lb[3]);  // LBA upper
    fis.add(9).write_volatile(lb[4]);  // LBA upper mid
    fis.add(10).write_volatile(lb[5]); // LBA upper high
    fis.add(11).write_volatile(0);     // Features high
    fis.add(12).write_volatile((sector_count & 0xFF) as u8);
    fis.add(13).write_volatile((sector_count >> 8) as u8);

    // PRDT: one entry pointing to buf, 1 sector = 512 bytes Max.
    // For multi-sector reads, use consecutive PRDT entries or larger buffer
    let prdt = ct_virt.add(0x80); // PRDT starts at offset 0x80 in command table
    let prdt_ptr = prdt as *mut u32;
    prdt_ptr.write_volatile(buf as u32);                      // DBA low
    prdt_ptr.add(1).write_volatile(0);                         // DBA high
    prdt_ptr.add(2).write_volatile(0);                         // reserved
    let prd_byte_count = (sector_count as u32).min(65535 / 512) * 512;
    prdt_ptr.add(3).write_volatile(prd_byte_count | (1 << 31)); // Byte count + interrupt

    // Write command header PRDBC (PRD Byte Count)
    let prdbc: u32 = 0; // Set after command completion by HBA (for reads)
    cl_hdr.add(2).write_volatile(0); // Clear PRDBC
    cl_hdr.add(3).write_volatile(prd_byte_count | (5 << 16)); // CFL=5, PRDBC=prd_byte_count

    // Issue command on slot 0
    port_write(mmio, port_idx, PORT_CI, 1);

    // Wait for completion
    for _ in 0..1000000 {
        let ci = port_read(mmio, port_idx, PORT_CI);
        let is_val = port_read(mmio, port_idx, PORT_IS);
        if (ci & 1) == 0 {
            // Clear interrupt status
            port_write(mmio, port_idx, PORT_IS, is_val);
            return sector_count;
        }
        if (is_val & (1 << 30)) != 0 { // Task File Error
            port_write(mmio, port_idx, PORT_IS, is_val);
            return 0;
        }
        core::hint::spin_loop();
    }
    0
}
