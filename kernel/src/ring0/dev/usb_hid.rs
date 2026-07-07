//! USB HID — xHCI controller detection (BIOS Legacy Emulation passthrough).
//!
//! v2.0: We do NOT take ownership of the xHCI from the BIOS and we do NOT
//! reset or restart the controller. We only discover it (BAR0 + read-only
//! capability register probes) for informational / panel purposes and leave
//! the BIOS in charge of USB.
//!
//! Rationale: the bmo desktop polls the legacy PS/2 ports 0x60/0x64 for
//! keyboard/mouse input. Those ports are serviced by the BIOS SMI handler
//! via USB Legacy Emulation while the BIOS owns the xHC. If we write
//! `USBLEGSUP_OS_SEM` (= take ownership) and then issue `USBCMD_HCRST`, the
//! BIOS stops emulating PS/2 and the host-controller reset drops every
//! currently-enumerated USB device — RGB keyboards/mice go dark, and we have
//! no real xHCI enumeration driver to bring them back. Hence: discovery
//! only, defer to BIOS.
//!
//! The register-layout constants and `xhci_write32` below are retained as
//! documentation for a future full xHCI driver; they are unused at runtime
//! today.

#![allow(dead_code)]

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static XHCI_MMIO_PHYS: AtomicUsize = AtomicUsize::new(0);
static XHCI_INITIALIZED: AtomicBool = AtomicBool::new(false);

unsafe fn xhci_read32(offset: u32) -> u32 {
    let base = XHCI_MMIO_PHYS.load(Ordering::Relaxed) as u64;
    if base == 0 { return 0xFFFFFFFF; }
    // Use identity mapping (MMIO BAR is below 4GB, already 1:1 mapped by UEFI)
    // phys_to_virt() only covers Usable RAM, not MMIO regions
    unsafe { core::ptr::read_volatile((base + offset as u64) as *const u32) }
}

unsafe fn xhci_write32(offset: u32, val: u32) {
    let base = XHCI_MMIO_PHYS.load(Ordering::Relaxed) as u64;
    if base == 0 { return; }
    unsafe { core::ptr::write_volatile((base + offset as u64) as *mut u32, val); }
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

/// Detect the xHCI controller (read-only) and report its capabilities.
///
/// This does NOT take ownership from the BIOS and does NOT reset/restart the
/// controller. USB Legacy Emulation stays active, so USB keyboards/mice keep
/// appearing as PS/2 on ports 0x60/0x64 — which is what the desktop polls.
///
/// `XHCI_INITIALIZED` is set to true once discovery succeeds so that
/// `is_initialized()` callers can tell we have a known xHCI controller under
/// BIOS management. See the module doc comment for the full rationale.
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

    // Read-only capability probing only. We must NOT write to the operational
    // registers or to the USBLEGSUP OS semaphore: doing so would either take
    // ownership from the BIOS (killing USB Legacy Emulation → 0x60/0x64 go
    // silent) or reset the controller (dropping all enumerated USB devices
    // and turning off RGB). The BIOS stays in charge of USB.
    unsafe {
        // 1. Capability length + structural params (informational)
        let cap_len = xhci_read32(CAPLENGTH) & 0xFF;
        let hcs1 = xhci_read32(HCSPARAMS1);
        let max_slots = (hcs1 & 0xFF).min(32);
        let hcc = xhci_read32(HCCPARAMS1);
        let eecp = ((hcc >> 8) & 0xFF) as u32;

        crate::dev::console::serial_write("[xhci] caplen=");
        crate::dev::console::serial_write_u64(cap_len as u64, 10);
        crate::dev::console::serial_write(" max_slots=");
        crate::dev::console::serial_write_u64(max_slots as u64, 10);
        crate::dev::console::serial_write(" eecp=0x");
        crate::dev::console::serial_write_u64(eecp as u64, 16);
        crate::dev::console::serial_write("\n");

        // 2. Report BIOS ownership status (read-only — never request ownership)
        if eecp >= 0x40 {
            let bios_sem = xhci_read32(eecp + USBLEGSUP_BIOS_SEM);
            let os_sem   = xhci_read32(eecp + USBLEGSUP_OS_SEM);
            let bios_owned = (bios_sem & 1) != 0;
            let os_owned   = (os_sem   & 1) != 0;
            crate::dev::console::serial_write("[xhci] Legacy Support: BIOS-owned=");
            crate::dev::console::serial_write(if bios_owned { "1" } else { "0" });
            crate::dev::console::serial_write(" OS-owned=");
            crate::dev::console::serial_write(if os_owned { "1" } else { "0" });
            crate::dev::console::serial_write("\n");
            if os_owned {
                // Someone (us, on a prior boot, or an earlier buggy version)
                // already took ownership. We can't safely hand it back, so
                // just log it — the desktop may still work if BIOS SMI is
                // still servicing ports 0x60/0x64 from a previous enumeration.
                crate::dev::console::serial_write("[xhci] WARN: OS already owns controller; Legacy Emulation may be off\n");
            }
        } else {
            crate::dev::console::serial_write("[xhci] No Legacy Support EECP — controller has no BIOS handoff\n");
        }

        // 3. Defer to BIOS — do NOT touch operational registers.
        crate::dev::console::serial_write("[xhci] Deferring to BIOS USB Legacy Emulation (0x60/0x64); no reset/ownership take\n");
        // Touch `op_base` only for a read-only status report; never write.
        let _op_base = cap_len;
        let _sts = xhci_read32(_op_base + USBSTS);
        let _ = _sts; // suppress unused-bindings warning in case of `deny`
    }

    XHCI_INITIALIZED.store(true, Ordering::Relaxed);
}
