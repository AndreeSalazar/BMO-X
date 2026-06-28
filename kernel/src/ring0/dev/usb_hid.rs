//! USB HID Detection & Minimal xHCI Stubs.
//!
//! Detects xHCI controllers via PCI and logs basic device info.
//! PS/2 emulation (via BIOS) handles keyboard/mouse input for now.
//! Full xHCI + HID Boot Protocol implementation is a future project.
//!
//! Architecture:
//!   - xHCI controller detected via PCI (class 0x0C, subclass 0x03)
//!   - MMIO registers mapped via phys_to_virt()
//!   - Capability registers read to identify controller version/ports
//!   - Connected USB devices logged for diagnostics
//!
//! PS/2 keyboard/mouse via BIOS emulation handles all input currently.
//! When full xHCI is implemented, this module will be the foundation.

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ── xHCI Register Offsets ──────────────────────────────────────────

/// xHCI MMIO base (physical address, set during init).
static XHCI_MMIO_PHYS: AtomicUsize = AtomicUsize::new(0);
static XHCI_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Read a 32-bit register from xHCI MMIO space.
unsafe fn xhci_read32(offset: u32) -> u32 {
    let base = XHCI_MMIO_PHYS.load(Ordering::Relaxed);
    if base == 0 { return 0; }
    let virt = crate::mm::vmm::phys_to_virt(base as u64);
    unsafe { core::ptr::read_volatile((virt + offset as u64) as *const u32) }
}

/// Write a 32-bit register to xHCI MMIO space.
unsafe fn xhci_write32(offset: u32, val: u32) {
    let base = XHCI_MMIO_PHYS.load(Ordering::Relaxed);
    if base == 0 { return; }
    let virt = crate::mm::vmm::phys_to_virt(base as u64);
    unsafe { core::ptr::write_volatile((virt + offset as u64) as *mut u32, val); }
}

// ── xHCI Capability Register Offsets ───────────────────────────────

const CAPLENGTH: u32 = 0x00;    // Capability Length
const HCIVERSION: u32 = 0x02;   // HCI Version (16-bit)
const HCSPARAMS1: u32 = 0x04;   // Structural Parameters 1
const HCSPARAMS2: u32 = 0x08;   // Structural Parameters 2
const HCSPARAMS3: u32 = 0x0C;   // Structural Parameters 3
const HCCPARAMS1: u32 = 0x10;   // Capability Parameters 1
const DBOFF: u32 = 0x14;        // Doorbell Offset
const RTSOFF: u32 = 0x18;       // Runtime Register Space Offset

/// Returns true if xHCI was detected and basic init succeeded.
pub fn is_initialized() -> bool {
    XHCI_INITIALIZED.load(Ordering::Relaxed)
}

// ── xHCI Initialization ────────────────────────────────────────────

/// Initialize USB HID detection via xHCI.
///
/// Called from phase2_dev after PCI scan. Detects xHCI controller,
/// reads capability registers, and logs device info.
///
/// This is non-fatal: if no xHCI is found or initialization fails,
/// PS/2 input continues to work via BIOS emulation.
pub fn init() {
    if XHCI_INITIALIZED.load(Ordering::Relaxed) { return; }

    // Find xHCI controller via PCI scan results
    let bar0 = match crate::dev::pcie::find_xhci_mmio() {
        Some(addr) => addr,
        None => {
            crate::dev::console::serial_write("[usb_hid] No xHCI controller found (PS/2 input available)\n");
            return;
        }
    };

    crate::dev::console::serial_write("[usb_hid] xHCI MMIO=0x");
    crate::dev::console::serial_write_u64(bar0, 16);
    crate::dev::console::serial_write("\n");

    XHCI_MMIO_PHYS.store(bar0 as usize, Ordering::Relaxed);

    // Read capability registers
    let cap_len = unsafe { xhci_read32(CAPLENGTH) } & 0xFF;
    let hci_ver = unsafe { xhci_read32(HCIVERSION) };
    let hcs_params1 = unsafe { xhci_read32(HCSPARAMS1) };
    let hcs_params2 = unsafe { xhci_read32(HCSPARAMS2) };
    let hcc_params1 = unsafe { xhci_read32(HCCPARAMS1) };

    let max_slots = hcs_params1 & 0xFF;
    let max_ports = (hcs_params1 >> 24) & 0xFF;
    let max_interrupts = ((hcs_params1 >> 10) & 0x3FF).max(1);
    let max_scratchpad = (hcs_params2 >> 21) & 0x1F;

    crate::dev::console::serial_write("[usb_hid] xHCI v");
    crate::dev::console::serial_write_u64(((hci_ver >> 8) & 0xFF) as u64, 10);
    crate::dev::console::serial_write(".");
    crate::dev::console::serial_write_u64((hci_ver & 0xFF) as u64, 10);
    crate::dev::console::serial_write(", cap_len=");
    crate::dev::console::serial_write_u64(cap_len as u64, 10);
    crate::dev::console::serial_write(", max_slots=");
    crate::dev::console::serial_write_u64(max_slots as u64, 10);
    crate::dev::console::serial_write(", max_ports=");
    crate::dev::console::serial_write_u64(max_ports as u64, 10);
    crate::dev::console::serial_write(", max_interrupts=");
    crate::dev::console::serial_write_u64(max_interrupts as u64, 10);
    crate::dev::console::serial_write("\n");

    if max_scratchpad > 0 {
        crate::dev::console::serial_write("[usb_hid] xHCI requires ");
        crate::dev::console::serial_write_u64(max_scratchpad as u64, 10);
        crate::dev::console::serial_write(" scratchpad buffers\n");
    }

    // Check if xHCI supports 64-bit addressing
    if hcc_params1 & (1 << 0) != 0 {
        crate::dev::console::serial_write("[usb_hid] xHCI supports 64-bit addressing\n");
    }

    // Check for context size flag (set to 64-byte contexts if bit 2)
    let ctx_size = if hcc_params1 & (1 << 2) != 0 { 64 } else { 32 };
    crate::dev::console::serial_write("[usb_hid] xHCI context size=");
    crate::dev::console::serial_write_u64(ctx_size as u64, 10);
    crate::dev::console::serial_write(" bytes\n");

    // NOTE: Full xHCI initialization (reset, configure, start) and
    // HID Boot Protocol device enumeration will be implemented in a
    // future version. For now, we detect and log the controller.
    // PS/2 emulation via BIOS handles all keyboard/mouse input.

    crate::dev::console::serial_write("[usb_hid] xHCI detected and logged. Full HID support pending.\n");
    crate::dev::console::serial_write("[usb_hid] PS/2 keyboard/mouse via BIOS emulation is active.\n");

    XHCI_INITIALIZED.store(true, Ordering::Relaxed);
}
