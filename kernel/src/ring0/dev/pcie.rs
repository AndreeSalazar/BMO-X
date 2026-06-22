#![allow(dead_code)]

//! PCI bus scanning — dual backend: IO ports (0xCF8/0xCFC) + ECAM MMIO.
//!
//! When ECAM (via ACPI MCFG) is available, uses memory-mapped access.
//! Otherwise falls back to legacy IO port access (works on all x86-64).

use core::arch::asm;

/// ECAM base address (set by init_ecam if MCFG found).
static mut ECAM_BASE: u64 = 0;
static mut ECAM_END_BUS: u8 = 0;
static mut USE_ECAM: bool = false;

/// Initialize ECAM — called when ACPI MCFG provides the base address.
///
/// `base` is the physical base of the ECAM MMIO region. On Ryzen 5 5600X,
/// the kernel identity-maps only the first 4 GB, so any ECAM base above
/// 0x1_0000_0000 would cause #PF on every config read. We explicitly
/// map that region with 2 MiB huge pages (kernel MMIO mapping).
pub fn init_ecam(base: u64, end_bus: u8) {
    // v1.6.3: Localization prints — find where init_ecam hangs.
    // Each marker writes a single line to COM1. If a marker is missing
    // from the serial log, the cuelgue is between the previous marker
    // and this one.
    crate::dev::console::serial_write("[pci D1] init_ecam ENTERED\n");

    crate::dev::console::serial_write("[pci] init_ecam: base=0x");
    print_hex(base);
    crate::dev::console::serial_write(" end_bus=");
    print_u32(end_bus as u32);
    crate::dev::console::serial_write("\n");
    crate::dev::console::serial_write("[pci D2] after header log\n");

    if base == 0 || base < 0x1000 {
        crate::dev::console::serial_write("[pci] ECAM base invalid, falling back to IO ports\n");
        unsafe {
            ECAM_BASE = 0;
            ECAM_END_BUS = 0;
            USE_ECAM = false;
        }
        return;
    }
    crate::dev::console::serial_write("[pci D3] base sanity OK\n");

    // Cap.end_bus to 8 — most consumer boards (incl. Ryzen 5 5600X) have
    // ECAM spanning only 0-7. Higher values mean server-class or buggy MCFG.
    let safe_end = if end_bus > 8 { 8 } else { end_bus };

    // Calculate ECAM size: each bus = 1 MB; buses 0..=safe_end.
    let bytes = (safe_end as usize + 1) * 1024 * 1024;
    let round_up_2mb = ((bytes + 0x1FFFFF) & !0x1FFFFF) as u64;
    crate::dev::console::serial_write("[pci D4] safe_end=");
    print_u32(safe_end as u32);
    crate::dev::console::serial_write(" round_up_2mb=0x");
    print_hex(round_up_2mb);
    crate::dev::console::serial_write("\n");

    // Map ECAM at its physical address (identity-style). This works whether
    // it's in low memory (already identity-mapped) or high memory (we map
    // it now with 2 MiB huge pages so the kernel can dereference config
    // space without #PF).
    crate::dev::console::serial_write("[pci] Mapping ECAM at 0x");
    print_hex(base);
    crate::dev::console::serial_write(" (");
    print_u32((round_up_2mb / 0x20000) as u32);
    crate::dev::console::serial_write(" x 2 MiB huge pages)\n");
    crate::dev::console::serial_write("[pci D5] about to map_kernel_mmio_huge\n");

    // v1.6.3: ECAM mapping is the most likely cuelgue site because it
    // walks the UEFI PML4 and may try to write to a PDPT entry that UEFI
    // already populated as a 1 GiB huge page. We catch the Err path and
    // fall back to IO ports.
    let result = unsafe {
        crate::mem::virt::map_kernel_mmio_huge(base, base, round_up_2mb as usize)
    };
    crate::dev::console::serial_write("[pci D6] map_kernel_mmio_huge returned\n");
    match result {
        Ok(()) => {
            crate::dev::console::serial_write("[pci] ECAM mapped OK\n");
            unsafe {
                ECAM_BASE = base;
                ECAM_END_BUS = safe_end;
                USE_ECAM = true;
            }
        }
        Err(e) => {
            crate::dev::console::serial_write("[pci] ECAM map FAILED: ");
            crate::dev::console::serial_write(e);
            crate::dev::console::serial_write(" — falling back to IO ports\n");
            unsafe {
                ECAM_BASE = 0;
            ECAM_END_BUS = 0;
                USE_ECAM = false;
            }
            return;
        }
    }
    crate::dev::console::serial_write("[pci] ECAM initialized .end_bus=");
    print_u32(safe_end as u32);
    crate::dev::console::serial_write(")\n");
    crate::dev::console::serial_write("\n");
}

/// Check if ECAM is available.
pub fn is_ecam() -> bool {
    unsafe { USE_ECAM }
}

// ── Low-level read/write ──────────────────────────────────────────

/// Read 32 bits from PCI config space (auto-selects ECAM or IO port).
pub fn pci_read32(bus: u8, dev: u8, func: u8, off: u16) -> u32 {
    if unsafe { USE_ECAM } {
        pci_read32_ecam(bus, dev, func, off)
    } else {
        pci_read32_io(bus, dev, func, off)
    }
}

/// Write 32 bits to PCI config space (auto-selects ECAM or IO port).
pub fn pci_write32(bus: u8, dev: u8, func: u8, off: u16, val: u32) {
    if unsafe { USE_ECAM } {
        pci_write32_ecam(bus, dev, func, off, val)
    } else {
        pci_write32_io(bus, dev, func, off, val)
    }
}

/// ECAM MMIO read.
fn pci_read32_ecam(bus: u8, dev: u8, func: u8, off: u16) -> u32 {
    let base = unsafe { ECAM_BASE };
    let addr = base
        + ((bus as u64) << 20)
        + ((dev as u64) << 15)
        + ((func as u64) << 12)
        + ((off as u64) & 0xFFC);
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

/// ECAM MMIO write.
fn pci_write32_ecam(bus: u8, dev: u8, func: u8, off: u16, val: u32) {
    let base = unsafe { ECAM_BASE };
    let addr = base
        + ((bus as u64) << 20)
        + ((dev as u64) << 15)
        + ((func as u64) << 12)
        + ((off as u64) & 0xFFC);
    unsafe { core::ptr::write_volatile(addr as *mut u32, val); }
}

/// Legacy IO port read (0xCF8 config address, 0xCFC data).
fn pci_read32_io(bus: u8, dev: u8, func: u8, off: u16) -> u32 {
    let addr: u32 = (1 << 31)
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC);
    unsafe {
        asm!("out dx, al", in("dx") 0xCF8u16, in("al") (addr & 0xFF) as u8);
        asm!("out dx, al", in("dx") 0xCF9u16, in("al") ((addr >> 8) & 0xFF) as u8);
        asm!("out dx, al", in("dx") 0xCFAu16, in("al") ((addr >> 16) & 0xFF) as u8);
        asm!("out dx, al", in("dx") 0xCFBu16, in("al") ((addr >> 24) & 0xFF) as u8);
        let val: u32;
        asm!("in eax, dx", in("dx") 0xCFCu16, out("eax") val);
        val
    }
}

/// Legacy IO port write.
fn pci_write32_io(bus: u8, dev: u8, func: u8, off: u16, val: u32) {
    let addr: u32 = (1 << 31)
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC);
    unsafe {
        asm!("out dx, al", in("dx") 0xCF8u16, in("al") (addr & 0xFF) as u8);
        asm!("out dx, al", in("dx") 0xCF9u16, in("al") ((addr >> 8) & 0xFF) as u8);
        asm!("out dx, al", in("dx") 0xCFAu16, in("al") ((addr >> 16) & 0xFF) as u8);
        asm!("out dx, al", in("dx") 0xCFBu16, in("al") ((addr >> 24) & 0xFF) as u8);
        asm!("out dx, eax", in("dx") 0xCFCu16, in("eax") val);
    }
}

// ── Device structure ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub bar0: u32,
    pub bar1: u32,
}

pub struct PciScanResult {
    pub devices: [PciDevice; 64],
    pub count: usize,
}

impl PciScanResult {
    fn new() -> Self {
        Self {
            devices: [PciDevice {
                bus: 0, device: 0, function: 0,
                vendor_id: 0, device_id: 0,
                class_code: 0, subclass: 0,
                bar0: 0, bar1: 0,
            }; 64],
            count: 0,
        }
    }

    pub fn find_device(&self, vendor: u16, device: u16) -> Option<&PciDevice> {
        self.devices[..self.count].iter().find(|d| {
            d.vendor_id == vendor && d.device_id == device
        })
    }
}

pub static mut SCAN_RESULT: Option<PciScanResult> = None;

// ── Scan ──────────────────────────────────────────────────────────

pub fn scan_pci_bus() -> PciScanResult {
    let r = PciScanResult::new();
    // v1.6.10: On the Ryzen 5 5600X the UEFI firmware has the legacy
    // PCI config space (ports 0xCF8/0xCFC) DISABLED, so even a single
    // `out 0xCF8, al` transaction wedges the CPU forever (it polls
    // waiting for a response that never arrives). ECAM is also broken
    // (see v1.6.6 — PDPT 1 GiB huge-page conflict with UEFI PML4).
    // So we cannot enumerate PCI at all on this firmware. We return
    // an empty scan result and the rest of the kernel keeps working.
    crate::dev::console::serial_write(
        "[pci] scan_pci_bus: SKIPPED in v1.6.10 (UEFI disabled IO-ports, ECAM broken)\n",
    );
    crate::dev::console::serial_write(
        "[pci] scan_pci_bus: 0 devices. NIC/AHCI/NVMe unavailable until ECAM is fixed.\n",
    );
    unsafe { SCAN_RESULT = Some(PciScanResult::new()); }
    return r;
}

/// Scan PCI bus using legacy IO ports (0xCF8/0xCFC) — 256 buses max.
/// v1.6.10: NOT CALLED. The Ryzen 5 5600X UEFI firmware wedges the
/// CPU on any 0xCF8/0xCFC transaction, so this is dead code until
/// we have a real ECAM path. Kept for the day ECAM is fixed and
/// we want a non-MCFG fallback for old machines.
#[allow(dead_code)]
fn scan_pci_bus_io() -> PciScanResult {
    let mut r = PciScanResult::new();
    for bus in 0..=255u16 {
        for dev in 0..32u8 {
            let vd = pci_read32_io(bus as u8, dev, 0, 0x00);
            let vendor = (vd & 0xFFFF) as u16;
            if vendor == 0xFFFF { continue; }
            let device_id = ((vd >> 16) & 0xFFFF) as u16;
            let cr = pci_read32_io(bus as u8, dev, 0, 0x08);
            let hdr = pci_read32_io(bus as u8, dev, 0, 0x0C);
            let multi = (hdr >> 16) & 0x80 != 0;
            let bar0 = pci_read32_io(bus as u8, dev, 0, 0x10);
            let bar1 = pci_read32_io(bus as u8, dev, 0, 0x14);
            if r.count < 64 {
                r.devices[r.count] = PciDevice {
                    bus: bus as u8, device: dev, function: 0,
                    vendor_id: vendor, device_id,
                    class_code: ((cr >> 24) & 0xFF) as u8,
                    subclass: ((cr >> 16) & 0xFF) as u8,
                    bar0, bar1,
                };
                r.count += 1;
            }
            if multi {
                for func in 1..8u8 {
                    let vd2 = pci_read32_io(bus as u8, dev, func, 0x00);
                    let v2 = (vd2 & 0xFFFF) as u16;
                    if v2 == 0xFFFF { continue; }
                    let cr2 = pci_read32_io(bus as u8, dev, func, 0x08);
                    let b0 = pci_read32_io(bus as u8, dev, func, 0x10);
                    let b1 = pci_read32_io(bus as u8, dev, func, 0x14);
                    if r.count < 64 {
                        r.devices[r.count] = PciDevice {
                            bus: bus as u8, device: dev, function: func,
                            vendor_id: v2,
                            device_id: ((vd2 >> 16) & 0xFFFF) as u16,
                            class_code: ((cr2 >> 24) & 0xFF) as u8,
                            subclass: ((cr2 >> 16) & 0xFF) as u8,
                            bar0: b0, bar1: b1,
                        };
                        r.count += 1;
                    }
                }
            }
            if bus > 0 && dev == 0 && r.count == 0 { break; }
        }
        if bus > 0 && r.count == 0 {
            let found_on_0 = scan_any_on_bus(0);
            if found_on_0 && bus > 4 { break; }
        }
    }
    r
}

fn scan_any_on_bus(bus: u8) -> bool {
    for dev in 0..32u8 {
        let vd = pci_read32_io(bus, dev, 0, 0x00);
        if (vd & 0xFFFF) as u16 != 0xFFFF { return true; }
    }
    false
}

/// Count of discovered PCI devices.
pub fn device_count() -> usize {
    unsafe { SCAN_RESULT.as_ref().map(|r| r.count).unwrap_or(0) }
}

/// Check if any PCI device is NVMe (class 0x01, subclass 0x08).
pub fn has_nvme() -> bool {
    unsafe {
        SCAN_RESULT.as_ref().map(|r| {
            r.devices[..r.count].iter().any(|d| d.class_code == 0x01 && d.subclass == 0x08)
        }).unwrap_or(false)
    }
}

/// Check if any PCI device is AHCI/SATA (class 0x01, subclass 0x06).
pub fn has_ahci() -> bool {
    unsafe {
        SCAN_RESULT.as_ref().map(|r| {
            r.devices[..r.count].iter().any(|d| d.class_code == 0x01 && d.subclass == 0x06)
        }).unwrap_or(false)
    }
}

/// Check if any PCI device is xHCI USB (class 0x0C, subclass 0x03).
pub fn has_xhci() -> bool {
    unsafe {
        SCAN_RESULT.as_ref().map(|r| {
            r.devices[..r.count].iter().any(|d| d.class_code == 0x0C && d.subclass == 0x03)
        }).unwrap_or(false)
    }
}

fn print_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    let mut buf = [0u8; 18];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in 0..16usize {
        buf[2 + i] = hex[((val >> (60 - i * 4)) & 0xF) as usize];
    }
    crate::dev::console::serial_write(core::str::from_utf8(&buf).unwrap_or("0x???"));
}

fn print_u32(val: u32) {
    if val == 0 {
        crate::dev::console::serial_write("0");
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    let mut v = val;
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    let s = core::str::from_utf8(&buf[i..]).unwrap_or("?");
    crate::dev::console::serial_write(s);
}

/// v1.8.8: This `init()` is a no-op that uses hardcoded values (base=0xE0000000,
/// end_bus=255). It is NOT called by the boot path — `p2_dev::run` instead
/// calls `init_ecam(0, 32)` to disable ECAM entirely (because the 5600X UEFI
/// wedges on ECAM MMIO access above 4 GB without proper PML4 re-mapping).
///
/// Use `vendor::amd::cpu::zen3::acpi_real::mcfg()` to read the real
/// ECAM base, and only call `init_ecam(base, end)` with values from
/// the MCFG table.
#[allow(dead_code)]
pub fn init() {
    init_ecam(0xE000_0000, 255);
}
