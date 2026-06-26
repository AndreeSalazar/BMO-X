use core::arch::asm;

/// PCI config space access — dual backend: IO ports + ECAM MMIO.
///
/// IO ports (0xCF8/0xCFC) work on most hardware.
/// ECAM is used when available via ACPI MCFG.

static mut USE_ECAM: bool = false;
static mut ECAM_BASE: u64 = 0;

/// Enable ECAM backend with given base address.
pub unsafe fn enable_ecam(base: u64) {
    ECAM_BASE = base;
    USE_ECAM = true;
}

/// Check if ECAM is active.
pub fn is_ecam() -> bool {
    unsafe { USE_ECAM }
}

/// Read 32 bits from PCI config space.
pub fn pci_read32(bus: u8, dev: u8, func: u8, off: u16) -> u32 {
    if unsafe { USE_ECAM } {
        pci_read32_ecam(bus, dev, func, off)
    } else {
        pci_read32_io(bus, dev, func, off)
    }
}

/// Write 32 bits to PCI config space.
pub fn pci_write32(bus: u8, dev: u8, func: u8, off: u16, val: u32) {
    if unsafe { USE_ECAM } {
        pci_write32_ecam(bus, dev, func, off, val)
    } else {
        pci_write32_io(bus, dev, func, off, val)
    }
}

/// Read 16 bits from PCI config space.
pub fn pci_read16(bus: u8, dev: u8, func: u8, off: u16) -> u16 {
    let val = pci_read32(bus, dev, func, off & !3);
    if off & 2 != 0 {
        (val >> 16) as u16
    } else {
        val as u16
    }
}

/// Read 8 bits from PCI config space.
pub fn pci_read8(bus: u8, dev: u8, func: u8, off: u16) -> u8 {
    let val = pci_read32(bus, dev, func, off & !3);
    ((val >> ((off & 3) * 8)) & 0xFF) as u8
}

// ── ECAM backend ──────────────────────────────────────────────

fn pci_read32_ecam(bus: u8, dev: u8, func: u8, off: u16) -> u32 {
    let base = unsafe { ECAM_BASE };
    let addr = base
        + ((bus as u64) << 20)
        + ((dev as u64) << 15)
        + ((func as u64) << 12)
        + ((off as u64) & 0xFFC);
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

fn pci_write32_ecam(bus: u8, dev: u8, func: u8, off: u16, val: u32) {
    let base = unsafe { ECAM_BASE };
    let addr = base
        + ((bus as u64) << 20)
        + ((dev as u64) << 15)
        + ((func as u64) << 12)
        + ((off as u64) & 0xFFC);
    unsafe { core::ptr::write_volatile(addr as *mut u32, val); }
}

// ── IO port backend ───────────────────────────────────────────

fn pci_read32_io(bus: u8, dev: u8, func: u8, off: u16) -> u32 {
    let addr: u32 = (1 << 31)
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC);
    unsafe {
        asm!("out dx, eax", in("dx") 0xCF8u16, in("eax") addr, options(nostack, preserves_flags));
        let val: u32;
        asm!("in eax, dx", in("dx") 0xCFCu16, out("eax") val, options(nostack, preserves_flags));
        val
    }
}

fn pci_write32_io(bus: u8, dev: u8, func: u8, off: u16, val: u32) {
    let addr: u32 = (1 << 31)
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC);
    unsafe {
        asm!("out dx, eax", in("dx") 0xCF8u16, in("eax") addr, options(nostack, preserves_flags));
        asm!("out dx, eax", in("dx") 0xCFCu16, in("eax") val, options(nostack, preserves_flags));
    }
}

// ── Helper: read BAR (32 or 64 bit) ──────────────────────────

/// Read a BAR register. If bit 0 is 0 and bits 1-2 are 10 (64-bit),
/// also read the upper 32 bits from BAR+4.
pub fn pci_read_bar(bus: u8, dev: u8, func: u8, bar_offset: u16) -> u64 {
    let lo = pci_read32(bus, dev, func, bar_offset);
    let bar_type = (lo >> 1) & 3;
    if bar_type == 2 {
        // 64-bit BAR
        let hi = pci_read32(bus, dev, func, bar_offset + 4);
        ((hi as u64) << 32) | ((lo as u64) & 0xFFFFFFF0)
    } else {
        (lo as u64) & if bar_type == 0 { 0xFFFFFFF0 } else { 0xFFFFFF01 }
    }
}

/// Read BAR5 (ABAR) for AHCI — always 64-bit memory BAR.
pub fn pci_read_bar5(bus: u8, dev: u8, func: u8) -> u64 {
    pci_read_bar(bus, dev, func, 0x24)
}
