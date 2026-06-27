//! NVMe Storage Driver (Ring 0 HAL).
//!
//! Manages NVM Express controllers. NVMe provides high-performance
//! block I/O via submission/completion queues in host memory.
//!
//! NVMe controller registers (BAR0 MMIO):
//!   - CAP: Controller Capabilities
//!   - VS: Version
//!   - CC: Controller Configuration
//!   - CSTS: Controller Status
//!   - AQA: Admin Queue Attributes
//!   - ASQ: Admin Submission Queue Base Address
//!   - ACQ: Admin Completion Queue Base Address
//!
//! NVMe queues:
//!   - Admin queue: Command/Completion for controller management
//!   - I/O queues: Command/Completion for data transfers (up to 64K queues)

use super::storage::StorageDevice;
use super::storage::StorageType;

/// NVMe controller register offsets.
const NVMe_CAP: usize = 0x00;    // Controller Capabilities (64-bit)
const NVMe_VS: usize = 0x08;     // Version
const NVMe_INTMS: usize = 0x0C;  // Interrupt Mask Set
const NVMe_INTMC: usize = 0x10;  // Interrupt Mask Clear
const NVMe_CC: usize = 0x14;     // Controller Configuration
const NVMe_CSTS: usize = 0x1C;   // Controller Status
const NVMe_AQA: usize = 0x24;    // Admin Queue Attributes
const NVMe_ASQ: usize = 0x28;    // Admin Submission Queue (64-bit)
const NVMe_ACQ: usize = 0x30;    // Admin Completion Queue (64-bit)

/// CC register bits.
const CC_EN: u32 = 1 << 0;   // Enable
const CC_CSS_NVM: u32 = 0 << 4; // Command Set Selected: NVM
const CC_MPS_SHIFT: u32 = 7; // Memory Page Size
const CC_AMS_RR: u32 = 0 << 11; // Arbitration: Round Robin
const CC_SHN_NONE: u32 = 0 << 14; // Shutdown Notification: None

/// CSTS register bits.
const CSTS_RDY: u32 = 1 << 0;  // Ready
const CSTS_CFS: u32 = 1 << 1;  // Controller Fatal Status
const CSTS_SHST: u32 = 3 << 2; // Shutdown Status

/// NVMe submission queue entry (64 bytes).
#[repr(C, align(64))]
pub struct NvmeSubmissionEntry {
    pub dword0: u32,       // CDW0: opcode, fused, PRP/SGL
    pub nsid: u32,         // Namespace ID
    pub cdw2: u32,
    pub cdw3: u32,
    pub metadata: u64,     // PRP Entry 1 (metadata)
    pub prp1: u64,         // PRP Entry 1 (data)
    pub prp2: u64,         // PRP Entry 2 (data)
    pub cdw10: u32,
    pub cdw11: u32,
    pub cdw12: u32,
    pub cdw13: u32,
    pub cdw14: u32,
    pub cdw15: u32,
}

/// NVMe completion queue entry (16 bytes).
#[repr(C, align(16))]
pub struct NvmeCompletionEntry {
    pub dword0: u32,     // Command-specific
    pub reserved: u32,
    pub sq_head: u16,    // Submission Queue Head Pointer
    pub sq_id: u16,      // Submission Queue Identifier
    pub cid: u16,        // Command Identifier
    pub status: u16,     // Status Field
}

/// NVMe namespace information.
#[derive(Debug, Clone, Copy)]
pub struct NvmeNamespace {
    pub nsid: u32,
    pub sectors_total: u64,
    pub sector_size: u32,
    pub lba_format: u8,
}

/// NVMe controller state.
#[derive(Debug)]
pub struct NvmeController {
    pub mmio_base: u64,
    pub irq: u8,
    pub cap: u64,
    pub vs: u32,
    pub page_size: u32,
    pub max_queue_entries: u32,
    pub admin_sq_phys: u64,
    pub admin_cq_phys: u64,
    pub namespaces: [NvmeNamespace; 16],
    pub ns_count: u8,
}

static mut CONTROLLER: Option<NvmeController> = None;

/// Read a 32-bit NVMe register.
unsafe fn nvme_read(mmio: u64, offset: usize) -> u32 {
    let ptr = (mmio + offset as u64) as *const u32;
    core::ptr::read_volatile(ptr)
}

/// Write a 32-bit NVMe register.
unsafe fn nvme_write(mmio: u64, offset: usize, val: u32) {
    let ptr = (mmio + offset as u64) as *mut u32;
    core::ptr::write_volatile(ptr, val);
}

/// Read a 64-bit NVMe register (CAP).
unsafe fn nvme_read64(mmio: u64, offset: usize) -> u64 {
    let ptr = (mmio + offset as u64) as *const u64;
    core::ptr::read_volatile(ptr)
}

/// Probe an NVMe controller at the given MMIO base address.
///
/// Steps:
///   1. Read controller capabilities
///   2. Disable controller (CC.EN = 0)
///   3. Set admin queue addresses
///   4. Enable controller (CC.EN = 1)
///   5. Wait for CSTS.RDY = 1
///   6. Identify namespaces
pub unsafe fn probe(mmio_base: u64, irq: u8) {
    crate::dev::console::serial_write("[nvme] probing MMIO=0x");
    crate::dev::console::serial_write_u64(mmio_base, 16);
    crate::dev::console::serial_write("\n");

    // Read capabilities
    let cap = nvme_read64(mmio_base, NVMe_CAP);
    let vs = nvme_read(mmio_base, NVMe_VS);
    let mps = (cap >> 48) & 0xF; // Memory Page Size
    let mqes = (cap >> 0) & 0xFFFF; // Max Queue Entries

    crate::dev::console::serial_write("[nvme] CAP: VS=");
    crate::dev::console::serial_write_u64(vs as u64, 16);
    crate::dev::console::serial_write(" MQES=");
    crate::dev::console::serial_write_u64(mqes as u64, 10);
    crate::dev::console::serial_write(" MPS=");
    crate::dev::console::serial_write_u64(mps, 10);
    crate::dev::console::serial_write("\n");

    // Disable controller if enabled
    let cc = nvme_read(mmio_base, NVMe_CC);
    if cc & CC_EN != 0 {
        nvme_write(mmio_base, NVMe_CC, cc & !CC_EN);
        let mut timeout = 100_000u32;
        while nvme_read(mmio_base, NVMe_CSTS) & CSTS_RDY != 0 && timeout > 0 {
            timeout -= 1;
        }
    }

    // Initialize controller struct
    let page_size = 4096u32 << mps;
    let mut ctrl = NvmeController {
        mmio_base,
        irq,
        cap,
        vs,
        page_size,
        max_queue_entries: mqes as u32 + 1,
        admin_sq_phys: 0,
        admin_cq_phys: 0,
        namespaces: [NvmeNamespace {
            nsid: 0,
            sectors_total: 0,
            sector_size: 0,
            lba_format: 0,
        }; 16],
        ns_count: 0,
    };

    // TODO: Allocate admin queues (SQ/CQ in physically contiguous memory)
    // TODO: Set AQA, ASQ, ACQ registers
    // TODO: Enable controller (CC.EN = 1)
    // TODO: Wait for CSTS.RDY = 1
    // TODO: Identify namespaces via Admin Identify command

    crate::dev::console::serial_write("[nvme] probe complete (queues not yet allocated)\n");

    CONTROLLER = Some(ctrl);
}

/// Get reference to the NVMe controller (if initialized).
pub fn controller() -> Option<&'static NvmeController> {
    unsafe { CONTROLLER.as_ref() }
}

/// Check if NVMe controller is initialized.
pub fn is_initialized() -> bool {
    unsafe { CONTROLLER.is_some() }
}
