//! USB HID — xHCI controller initialization + device detection.
//!
//! v1.9: Takes ownership from BIOS, resets and starts the controller.
//! This powers up USB ports — RGB keyboards should light up immediately.

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static XHCI_MMIO_PHYS: AtomicUsize = AtomicUsize::new(0);
static XHCI_INITIALIZED: AtomicBool = AtomicBool::new(false);

unsafe fn xhci_read32(offset: u32) -> u32 {
    let base = XHCI_MMIO_PHYS.load(Ordering::Relaxed);
    if base == 0 { return 0xFFFFFFFF; }
    let virt = crate::mm::vmm::phys_to_virt(base as u64);
    unsafe { core::ptr::read_volatile((virt + offset as u64) as *const u32) }
}

unsafe fn xhci_write32(offset: u32, val: u32) {
    let base = XHCI_MMIO_PHYS.load(Ordering::Relaxed);
    if base == 0 { return; }
    let virt = crate::mm::vmm::phys_to_virt(base as u64);
    unsafe { core::ptr::write_volatile((virt + offset as u64) as *mut u32, val); }
}

// ── Capability Registers (offset from MMIO base) ───────────────────

const CAPLENGTH:   u32 = 0x00;
const HCSPARAMS1:  u32 = 0x04;
const HCSPARAMS2:  u32 = 0x08;
const HCCPARAMS1:  u32 = 0x10;

// ── Operational Registers (offset = cap_length) ────────────────────

const USBCMD:   u32 = 0x00; // USB Command
const USBSTS:   u32 = 0x04; // USB Status
const CONFIG:   u32 = 0x38; // Configure

// ── Legacy Support (offsets from MMIO base, EECP-based) ────────────
// Standard EECP offset: HCCPARAMS1 bits 15:8
const USBLEGSUP_BIOS_SEM: u32 = 0x00; // offset within EECP block
const USBLEGSUP_OS_SEM:   u32 = 0x04;

const USBCMD_RS:    u32 = 1 << 0;  // Run/Stop
const USBCMD_HCRST: u32 = 1 << 1;  // Host Controller Reset
const USBSTS_HCH:   u32 = 1 << 0;  // HCHalted
const USBSTS_CNR:   u32 = 1 << 11; // Controller Not Ready

pub fn is_initialized() -> bool {
    XHCI_INITIALIZED.load(Ordering::Relaxed)
}

/// Initialize the xHCI controller: take ownership, reset, start.
/// This powers up USB ports — keyboard/mouse may get RGB and basic power.
pub fn init() {
    if XHCI_INITIALIZED.load(Ordering::Relaxed) { return; }

    let bar0 = match crate::dev::pcie::find_xhci_mmio() {
        Some(addr) => addr,
        None => {
            crate::dev::console::serial_write("[xhci] No xHCI controller found\n");
            return;
        }
    };

    crate::dev::console::serial_write("[xhci] MMIO=0x");
    crate::dev::console::serial_write_u64(bar0, 16);
    crate::dev::console::serial_write("\n");

    XHCI_MMIO_PHYS.store(bar0 as usize, Ordering::Relaxed);

    unsafe {
        // 1. Take ownership from BIOS (if EECP present)
        let hcc = xhci_read32(HCCPARAMS1);
        let eecp = ((hcc >> 8) & 0xFF) as u32;
        if eecp >= 0x40 {
            let bios_sem = xhci_read32(eecp + USBLEGSUP_BIOS_SEM);
            if (bios_sem & 1) != 0 {
                // BIOS owns the controller — request ownership
                xhci_write32(eecp + USBLEGSUP_OS_SEM, 1);
                // Wait for BIOS to release (reads back as 0)
                for _ in 0..100000 {
                    let bios = xhci_read32(eecp + USBLEGSUP_BIOS_SEM);
                    if (bios & 1) == 0 { break; }
                    core::hint::spin_loop();
                }
                crate::dev::console::serial_write("[xhci] Ownership taken from BIOS\n");
            }
        }

        // 2. Wait for CNR (Controller Not Ready) to clear
        let cap_len = xhci_read32(CAPLENGTH) & 0xFF;
        let op_base = cap_len;
        let mut timeout = 50000;
        while (xhci_read32(op_base + USBSTS) & USBSTS_CNR) != 0 && timeout > 0 {
            timeout -= 1;
            core::hint::spin_loop();
        }
        if timeout == 0 {
            crate::dev::console::serial_write("[xhci] WARN: CNR timeout\n");
        }

        // 3. Stop the controller (clear RS bit)
        let cmd = xhci_read32(op_base + USBCMD);
        xhci_write32(op_base + USBCMD, cmd & !USBCMD_RS);
        // Wait for HCHalted
        for _ in 0..50000 {
            if (xhci_read32(op_base + USBSTS) & USBSTS_HCH) != 0 { break; }
            core::hint::spin_loop();
        }

        // 4. Reset the controller
        xhci_write32(op_base + USBCMD, USBCMD_HCRST);
        for _ in 0..100000 {
            if (xhci_read32(op_base + USBCMD) & USBCMD_HCRST) == 0 { break; }
            if (xhci_read32(op_base + USBSTS) & USBSTS_CNR) == 0 { break; }
            core::hint::spin_loop();
        }
        crate::dev::console::serial_write("[xhci] Controller reset complete\n");

        // 5. Wait for CNR again after reset
        timeout = 50000;
        while (xhci_read32(op_base + USBSTS) & USBSTS_CNR) != 0 && timeout > 0 {
            timeout -= 1;
            core::hint::spin_loop();
        }

        // 6. Program max device slots (from HCSPARAMS1 bits 7:0)
        let max_slots = (xhci_read32(HCSPARAMS1) & 0xFF).min(32);
        let config_val = xhci_read32(op_base + CONFIG) & !0xFF;
        xhci_write32(op_base + CONFIG, config_val | max_slots);
        crate::dev::console::serial_write("[xhci] Max slots configured: ");
        crate::dev::console::serial_write_u64(max_slots as u64, 10);
        crate::dev::console::serial_write("\n");

        // 7. Start the controller (set RS bit)
        let cmd = xhci_read32(op_base + USBCMD);
        xhci_write32(op_base + USBCMD, cmd | USBCMD_RS);
        // Wait for HCHalted to clear (controller is running)
        for _ in 0..50000 {
            if (xhci_read32(op_base + USBSTS) & USBSTS_HCH) == 0 { break; }
            core::hint::spin_loop();
        }
        crate::dev::console::serial_write("[xhci] Controller started — USB ports powered\n");
    }

    XHCI_INITIALIZED.store(true, Ordering::Relaxed);
}
