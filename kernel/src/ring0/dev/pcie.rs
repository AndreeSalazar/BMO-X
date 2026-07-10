
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
        crate::mm::virt::map_kernel_mmio_huge(base, base, round_up_2mb as usize)
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
        // PCI config address must be written as one 32-bit transaction to
        // 0xCF8. Do NOT split this into byte writes to 0xCF8..0xCFB: on PC
        // hardware 0xCF9 is the reset-control register, so touching it can
        // instantly reboot the machine during PCI/GPU probing.
        asm!("out dx, eax", in("dx") 0xCF8u16, in("eax") addr, options(nostack, preserves_flags));
        let val: u32;
        asm!("in eax, dx", in("dx") 0xCFCu16, out("eax") val, options(nostack, preserves_flags));
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
        // Same safety rule as read: a single 32-bit write to 0xCF8 only.
        // 0xCF9 is not PCI address byte 1; it is platform reset control.
        asm!("out dx, eax", in("dx") 0xCF8u16, in("eax") addr, options(nostack, preserves_flags));
        asm!("out dx, eax", in("dx") 0xCFCu16, in("eax") val, options(nostack, preserves_flags));
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

#[derive(Clone, Copy, Debug)]
pub struct PciScanResult {
    pub devices: [PciDevice; 64],
    pub count: usize,
}

impl PciScanResult {
    pub const fn empty() -> Self {
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

    fn new() -> Self {
        Self::empty()
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
    if unsafe { USE_ECAM } {
        crate::dev::console::serial_write("[pci] scan_pci_bus: scanning via ECAM MMIO...\n");
    } else {
        crate::dev::console::serial_write("[pci] scan_pci_bus: scanning via legacy IO ports (0xCF8/0xCFC)...\n");

        // Probe: try reading vendor ID from Bus 0, Dev 0, Fn 0
        let probe = pci_read32_io(0, 0, 0, 0x00);
        let vendor = (probe & 0xFFFF) as u16;
        if vendor == 0xFFFF {
            // No device at Bus 0 Dev 0 — but IO ports might still work
            let probe2 = pci_read32_io(0, 1, 0, 0x00);
            let v2 = (probe2 & 0xFFFF) as u16;
            if v2 == 0xFFFF {
                crate::dev::console::serial_write("[pci] IO ports: no devices found on Bus 0 (probe=0x");
                print_hex(probe as u64);
                crate::dev::console::serial_write(")\n");
            }
        }
        crate::dev::console::serial_write("[pci] IO probe completed, starting scan...\n");
    }

    let r = scan_pci_bus_internal();
    crate::dev::console::serial_write("[pci] scan complete: ");
    print_u32(r.count as u32);
    crate::dev::console::serial_write(" devices found\n");

    // Store permanently in SCAN_RESULT so subsequent queries work
    unsafe { SCAN_RESULT = Some(r); }

    // Log each device found
    unsafe {
        if let Some(ref res) = SCAN_RESULT {
            for i in 0..res.count {
                let d = &res.devices[i];
                crate::dev::console::serial_write("  [");
                print_u32(i as u32);
                crate::dev::console::serial_write("] PCI ");
                print_u32(d.bus as u32);
                crate::dev::console::serial_write(":");
                print_u32(d.device as u32);
                crate::dev::console::serial_write(".");
                print_u32(d.function as u32);
                crate::dev::console::serial_write(" VD=0x");
                print_hex(((d.device_id as u64) << 16) | (d.vendor_id as u64));
                crate::dev::console::serial_write(" class=");
                print_u32(d.class_code as u32);
                crate::dev::console::serial_write("/");
                print_u32(d.subclass as u32);
                crate::dev::console::serial_write(" BAR0=0x");
                print_hex(d.bar0 as u64);
                crate::dev::console::serial_write("\n");
            }
        }
    }

    unsafe { SCAN_RESULT.clone().unwrap() }
}

/// Safe internal PCI scanning using generic pci_read32 (auto-selects ECAM/IO ports).
fn scan_pci_bus_internal() -> PciScanResult {
    let mut r = PciScanResult::new();
    let max_bus = if unsafe { USE_ECAM } { unsafe { ECAM_END_BUS as u16 } } else { 15 };

    for bus in 0..=max_bus {
        for dev in 0..32u8 {
            let vd = pci_read32(bus as u8, dev, 0, 0x00);
            let vendor = (vd & 0xFFFF) as u16;
            if vendor == 0xFFFF { continue; }
            let device_id = ((vd >> 16) & 0xFFFF) as u16;
            let cr = pci_read32(bus as u8, dev, 0, 0x08);
            let hdr = pci_read32(bus as u8, dev, 0, 0x0C);
            let multi = (hdr >> 16) & 0x80 != 0;
            let bar0 = pci_read32(bus as u8, dev, 0, 0x10);
            let bar1 = pci_read32(bus as u8, dev, 0, 0x14);
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
                    let vd2 = pci_read32(bus as u8, dev, func, 0x00);
                    let v2 = (vd2 & 0xFFFF) as u16;
                    if v2 == 0xFFFF { continue; }
                    let cr2 = pci_read32(bus as u8, dev, func, 0x08);
                    let b0 = pci_read32(bus as u8, dev, func, 0x10);
                    let b1 = pci_read32(bus as u8, dev, func, 0x14);
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
        let vd = pci_read32(bus, dev, 0, 0x00);
        if (vd & 0xFFFF) as u16 != 0xFFFF { return true; }
    }
    false
}

/// Count of discovered PCI devices.
pub fn device_count() -> usize {
    unsafe { SCAN_RESULT.as_ref().map(|r| r.count).unwrap_or(0) }
}

/// Check if any PCI device is AHCI/SATA (class 0x01, subclass 0x06).
pub fn has_ahci() -> bool {
    unsafe {
        SCAN_RESULT.as_ref().map(|r| {
            r.devices[..r.count].iter().any(|d| d.class_code == 0x01 && d.subclass == 0x06)
        }).unwrap_or(false)
    }
}

/// Find the AHCI controller and return its MMIO base address (BAR5).
/// AHCI controllers use BAR5 (ABAR) for memory-mapped registers.
/// Returns None if no AHCI controller found.
pub fn find_ahci_mmio() -> Option<u64> {
    unsafe {
        SCAN_RESULT.as_ref().and_then(|r| {
            r.devices[..r.count].iter().find(|d| {
                d.class_code == 0x01 && d.subclass == 0x06
            }).map(|d| {
                // BAR5 is at PCI config offset 0x24
                let bar5_lo = pci_read32(d.bus, d.device, d.function, 0x24);
                let bar5_hi = pci_read32(d.bus, d.device, d.function, 0x28);
                let bar5 = ((bar5_hi as u64) << 32) | (bar5_lo as u64);
                // Mask off BAR type bits (bit 0 = I/O, bit 1 = 64-bit)
                let mmio = bar5 & !0xF_u64;
                crate::dev::console::serial_write("[pci] AHCI ABAR=0x");
                print_hex(mmio);
                crate::dev::console::serial_write("\n");
                mmio
            })
        })
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

/// Find the xHCI controller and return its MMIO base address (BAR0).
/// xHCI controllers use BAR0 for memory-mapped operational registers.
/// Returns None if no xHCI controller found.
pub fn find_xhci_mmio() -> Option<u64> {
    unsafe {
        SCAN_RESULT.as_ref().and_then(|r| {
            r.devices[..r.count].iter().find(|d| {
                d.class_code == 0x0C && d.subclass == 0x03
            }).map(|d| {
                // BAR0 is at PCI config offset 0x10
                let bar0_lo = pci_read32(d.bus, d.device, d.function, 0x10);
                // Bit 2:1 = 00 → 32-bit MMIO, 10 → 64-bit MMIO
                let bar0: u64 = if (bar0_lo & 0x04) != 0 {
                    let bar0_hi = pci_read32(d.bus, d.device, d.function, 0x14);
                    ((bar0_hi as u64) << 32) | (bar0_lo & !0x0F) as u64
                } else {
                    (bar0_lo & !0x0F) as u64
                };
                let mmio = bar0;
                crate::dev::console::serial_write("[pci] xHCI BAR0=0x");
                print_hex(mmio);
                crate::dev::console::serial_write("\n");
                mmio
            })
        })
    }
}

/// Find ALL XHCI controllers and return their MMIO BAR0 addresses.
/// On AMD platforms there are TWO controllers: CPU SoC + chipset (Promontory/A320).
pub fn find_all_xhci_mmio() -> (Option<u64>, Option<u64>) {
    let mut first = None;
    let mut second = None;
    unsafe {
        if let Some(r) = SCAN_RESULT.as_ref() {
            crate::dev::console::serial_write("[pci] find_all_xhci: scanning ");
            print_u32(r.count as u32);
            crate::dev::console::serial_write(" devices\n");
            for d in &r.devices[..r.count] {
                if d.class_code == 0x0C && d.subclass == 0x03 {
                    let bar0_lo = pci_read32(d.bus, d.device, d.function, 0x10);
                    let bar0: u64 = if (bar0_lo & 0x04) != 0 {
                        let bar0_hi = pci_read32(d.bus, d.device, d.function, 0x14);
                        ((bar0_hi as u64) << 32) | (bar0_lo & !0x0F) as u64
                    } else {
                        (bar0_lo & !0x0F) as u64
                    };
                    crate::dev::console::serial_write("[pci] xHCI found: bus=");
                    print_u32(d.bus as u32);
                    crate::dev::console::serial_write(" dev=");
                    print_u32(d.device as u32);
                    crate::dev::console::serial_write(" fn=");
                    print_u32(d.function as u32);
                    crate::dev::console::serial_write(" BAR0=0x");
                    print_hex(bar0);
                    crate::dev::console::serial_write("\n");
                    if first.is_none() {
                        first = Some(bar0);
                    } else if second.is_none() {
                        second = Some(bar0);
                    }
                }
            }
        } else {
            crate::dev::console::serial_write("[pci] find_all_xhci: SCAN_RESULT is None!\n");
        }
    }
    crate::dev::console::serial_write("[pci] find_all_xhci: returning (");
    print_u32(first.is_some() as u32);
    crate::dev::console::serial_write(", ");
    print_u32(second.is_some() as u32);
    crate::dev::console::serial_write(")\n");
    (first, second)
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

/// Generic: find a PCI device by class/subclass and return its BAR0 MMIO base.
/// Correctly handles 32-bit vs 64-bit BARs per PCI spec.
pub fn find_device_mmio(class_code: u8, subclass: u8) -> Option<u64> {
    unsafe {
        SCAN_RESULT.as_ref().and_then(|r| {
            r.devices[..r.count].iter().find(|d| {
                d.class_code == class_code && d.subclass == subclass
            }).map(|d| {
                let bar0_lo = pci_read32(d.bus, d.device, d.function, 0x10);
                if bar0_lo & 0x04 != 0 {
                    // 64-bit BAR: combine BAR0 (low) + BAR1 (high)
                    let bar0_hi = pci_read32(d.bus, d.device, d.function, 0x14);
                    ((bar0_hi as u64) << 32) | ((bar0_lo & !0xF) as u64)
                } else {
                    // 32-bit BAR: only BAR0
                    (bar0_lo & !0xF) as u64
                }
            })
        })
    }
}
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
