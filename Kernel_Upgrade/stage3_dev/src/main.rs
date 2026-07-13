#![no_std]
#![no_main]
#![allow(dead_code)]

use core::panic::PanicInfo;
use core::arch::asm;
use boot_context::{BootContext, PciDevice};

// ═══════════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════════

const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;
const COM1: u16 = 0x3F8;
const PAGE_SIZE: u64 = 4096;

fn phys_to_virt(phys: u64) -> u64 { phys + HIGH_MEM_BASE }

// ═══════════════════════════════════════════════════════════════════════════
// Serial I/O
// ═══════════════════════════════════════════════════════════════════════════

fn outb(port: u16, val: u8) { unsafe { asm!("out dx, al", in("dx") port, in("al") val); } }
fn inb(port: u16) -> u8 { let v: u8; unsafe { asm!("in al, dx", in("dx") port, out("al") v); } v }
fn inl(port: u16) -> u32 { let v: u32; unsafe { asm!("in eax, dx", in("dx") port, out("eax") v); } v }
fn outl(port: u16, val: u32) { unsafe { asm!("out dx, eax", in("dx") port, in("eax") val); } }

fn serial_write_byte(b: u8) {
    let mut timeout = 100_000u32;
    while inb(COM1 + 5) & 0x20 == 0 {
        timeout = timeout.saturating_sub(1);
        if timeout == 0 { return; }
    }
    outb(COM1, b);
}

fn serial_write(s: &str) {
    for b in s.bytes() {
        if b == b'\n' { serial_write_byte(b'\r'); }
        serial_write_byte(b);
    }
}

fn serial_write_u64(value: u64, min_width: usize) {
    if value == 0 {
        for _ in 0..min_width.min(1) { serial_write_byte(b'0'); }
        if min_width == 0 { serial_write_byte(b'0'); }
        return;
    }
    let mut buf = [0u8; 16];
    let mut i = 0;
    let mut v = value;
    while v > 0 {
        let digit = (v & 0xF) as u8;
        buf[i] = if digit < 10 { b'0' + digit } else { b'a' + digit - 10 };
        v >>= 4;
        i += 1;
    }
    while i < min_width && i < buf.len() { buf[i] = b'0'; i += 1; }
    while i > 0 { i -= 1; serial_write_byte(buf[i]); }
}

fn serial_write_hex32(v: u32) {
    serial_write_u64(v as u64, 8);
}

// ═══════════════════════════════════════════════════════════════════════════
// ACPI RSDP/XSDT/MCFG parsing (standalone, no dependencies)
// ═══════════════════════════════════════════════════════════════════════════

#[repr(C, packed)]
struct RsdpHeader {
    signature: [u8; 8],
    checksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_addr: u32,
    length: u32,
    xsdt_addr: u64,
    extended_checksum: u8,
    _reserved: [u8; 3],
}

#[repr(C, packed)]
struct AcpiSdtHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: [u8; 4],
    creator_revision: u32,
}

const RSDP_SIG: [u8; 8] = *b"RSD PTR ";
const MCFG_SIG: [u8; 4] = *b"MCFG";
const HPET_SIG: [u8; 4] = *b"HPET";
const MADT_SIG: [u8; 4] = *b"APIC";
const FADT_SIG: [u8; 4] = *b"FACP";

fn acpi_checksum(addr: u64, len: usize) -> bool {
    let ptr = addr as *const u8;
    let mut sum: u8 = 0;
    for i in 0..len { unsafe { sum = sum.wrapping_add(ptr.add(i).read()); } }
    sum == 0
}

fn acpi_find_rsdp(ctx: &BootContext) -> Option<u64> {
    if ctx.rsdp != 0 { return Some(ctx.rsdp); }
    // Scan EBDA
    let ebda_seg = unsafe { (0x40E as *const u16).read() as u64 };
    let ebda_start = ebda_seg << 4;
    for addr in (ebda_start..ebda_start + 1024).step_by(16) {
        if acpi_check_rsdp(addr) { return Some(addr); }
    }
    // Scan BIOS ROM
    for addr in (0xE0000..0x100000).step_by(16) {
        if acpi_check_rsdp(addr) { return Some(addr); }
    }
    None
}

fn acpi_check_rsdp(addr: u64) -> bool {
    let ptr = addr as *const u8;
    for i in 0..8 { unsafe { if ptr.add(i).read() != RSDP_SIG[i] { return false; } } }
    let rev = unsafe { ptr.add(15).read() };
    let len: usize = if rev >= 2 { unsafe { ptr.add(20).read() as usize } } else { 20 };
    acpi_checksum(addr, len)
}

fn acpi_find_table(rsdp_addr: u64, sig: &[u8; 4]) -> Option<(u64, u32)> {
    unsafe {
        let rev = (rsdp_addr as *const u8).add(15).read();
        let xsdt_addr = if rev >= 2 {
            let ptr = rsdp_addr as *const u64;
            let len_ptr = (rsdp_addr + 16) as *const u8;
            if len_ptr.read() >= 24 { ptr.add(3).read() } else { 0 }
        } else {
            (rsdp_addr as *const u32).add(2).read() as u64
        };
        if xsdt_addr == 0 { return None; }

        let xsdt = xsdt_addr as *const AcpiSdtHeader;
        let entry_count = ((*xsdt).length as usize - core::mem::size_of::<AcpiSdtHeader>()) / 8;
        for i in 0..entry_count {
            let entry_ptr = xsdt_addr + core::mem::size_of::<AcpiSdtHeader>() as u64 + (i * 8) as u64;
            let tbl_addr = (entry_ptr as *const u64).read();
            let tbl = tbl_addr as *const AcpiSdtHeader;
            let mut tbl_sig = [0u8; 4];
            for j in 0..4 { tbl_sig[j] = (*tbl).signature[j]; }
            if &tbl_sig == sig {
                return Some((tbl_addr, (*tbl).length));
            }
        }
    }
    None
}

struct AcpiInfo {
    rsdp: u64,
    mcfg_base: Option<u64>,
    mcfg_end_bus: Option<u8>,
    hpet_base: Option<u64>,
    ioapic_base: Option<u64>,
    lapic_addr: Option<u64>,
    pm_timer_port: Option<u16>,
}

fn acpi_init(ctx: &BootContext) -> AcpiInfo {
    let mut info = AcpiInfo {
        rsdp: 0,
        mcfg_base: None,
        mcfg_end_bus: None,
        hpet_base: None,
        ioapic_base: None,
        lapic_addr: None,
        pm_timer_port: None,
    };

    let rsdp = match acpi_find_rsdp(ctx) {
        Some(a) => { info.rsdp = a; a }
        None => { serial_write("[acpi] RSDP not found\n"); return info; }
    };

    serial_write("[acpi] RSDP at 0x");
    serial_write_u64(rsdp, 16);
    serial_write("\n");

    // Parse MCFG for PCIe ECAM base
    if let Some((addr, _len)) = acpi_find_table(rsdp, &MCFG_SIG) {
        unsafe {
            let entries = ((*(addr as *const AcpiSdtHeader)).length as usize - 44) / 16;
            if entries > 0 {
                let entry = (addr + 44) as *const u64;
                let base = entry.read();
                let bus_end = ((addr + 52) as *const u8).read();
                info.mcfg_base = Some(base);
                info.mcfg_end_bus = Some(bus_end);
                serial_write("[acpi] MCFG: ECAM at 0x");
                serial_write_u64(base, 16);
                serial_write(", bus 0-");
                serial_write_u64(bus_end as u64, 10);
                serial_write("\n");
            }
        }
    }

    // Parse HPET table
    if let Some((addr, _len)) = acpi_find_table(rsdp, &HPET_SIG) {
        unsafe {
            let hpet_base = (addr + 40 + 8) as *const u64; // Event timer block base
            let base = hpet_base.read();
            info.hpet_base = Some(base);
            serial_write("[acpi] HPET base at 0x");
            serial_write_u64(base, 16);
            serial_write("\n");
        }
    }

    // Parse MADT for APIC info
    if let Some((addr, _len)) = acpi_find_table(rsdp, &MADT_SIG) {
        unsafe {
            let lapic_addr = (addr + 36) as *const u32; // Local APIC address
            info.lapic_addr = Some(lapic_addr.read() as u64);
            serial_write("[acpi] LAPIC at 0x");
            serial_write_u64(lapic_addr.read() as u64, 16);
            serial_write("\n");

            let body = addr + 44;
            let len = (*(addr as *const AcpiSdtHeader)).length as usize;
            let mut offset = 44usize;
            while offset + 2 <= len {
                let etype = (body + offset as u64) as *const u8;
                let elen = (body + offset as u64 + 1) as *const u8;
                if *etype == 1 { // I/O APIC
                    let ioapic_addr = (body + offset as u64 + 4) as *const u32;
                    let ioapic_base = ioapic_addr.read() as u64;
                    info.ioapic_base = Some(ioapic_base);
                    serial_write("[acpi] I/O APIC at 0x");
                    serial_write_u64(ioapic_base, 16);
                    serial_write("\n");
                }
                offset += *elen as usize;
            }
        }
    }

    // Parse FADT for PM timer
    if let Some((addr, _len)) = acpi_find_table(rsdp, &FADT_SIG) {
        unsafe {
            let pm_tmr = (addr + 76) as *const u32; // PM_TMR_BLK
            let blk = pm_tmr.read() as u16;
            if blk != 0 {
                info.pm_timer_port = Some(blk);
                serial_write("[acpi] PM timer at port 0x");
                serial_write_u64(blk as u64, 4);
                serial_write("\n");
            }
        }
    }

    info
}

// ═══════════════════════════════════════════════════════════════════════════
// PCI Enumeration (IO ports + ECAM)
// ═══════════════════════════════════════════════════════════════════════════

const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

fn pci_read32_io(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    let addr = 0x80000000u32 | ((bus as u32) << 16) | ((dev as u32) << 11) | ((func as u32) << 8) | (off as u32 & 0xFC);
    unsafe { outl(PCI_CONFIG_ADDR, addr); inl(PCI_CONFIG_DATA) }
}

fn pci_read32_ecam(base: u64, bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    let addr = base + ((bus as u64) << 20) | ((dev as u64) << 15) | ((func as u64) << 12) | (off as u64);
    let ptr = phys_to_virt(addr) as *const u32;
    unsafe { ptr.read_volatile() }
}

fn pci_enumerate(ctx: &mut BootContext, acpi: &AcpiInfo) {
    serial_write("[pci] Enumerating PCI bus\n");
    let mut count = 0u32;
    let use_ecam = acpi.mcfg_base.is_some();
    let ecam_base = acpi.mcfg_base.unwrap_or(0);
    let max_bus = acpi.mcfg_end_bus.unwrap_or(0xFF);

    for bus in 0u16..=max_bus as u16 {
        for device in 0..32u16 {
            let vendor = if use_ecam {
                pci_read32_ecam(ecam_base, bus as u8, device as u8, 0, 0) as u16
            } else {
                pci_read32_io(bus as u8, device as u8, 0, 0) as u16
            };
            if vendor == 0xFFFF { continue; }

            let reg0 = if use_ecam { pci_read32_ecam(ecam_base, bus as u8, device as u8, 0, 0) } else { pci_read32_io(bus as u8, device as u8, 0, 0) };
            let reg2 = if use_ecam { pci_read32_ecam(ecam_base, bus as u8, device as u8, 0, 8) } else { pci_read32_io(bus as u8, device as u8, 0, 8) };
            let vid = reg0 as u16;
            let did = (reg0 >> 16) as u16;
            let class = ((reg2 >> 24) & 0xFF) as u8;
            let subclass = ((reg2 >> 16) & 0xFF) as u8;
            let bar0 = reg0 << 16;

            if count < 32 {
                ctx.pci_devices[count as usize] = PciDevice {
                    bus: bus as u8, device: device as u8, function: 0,
                    class, subclass, vendor_id: vid, device_id: did, bar0,
                };
            }
            count += 1;

            serial_write("[pci] ");
            serial_write_u64(bus as u64, 2); serial_write(":");
            serial_write_u64(device as u64, 2); serial_write(" (");
            serial_write_u64(class as u64, 2); serial_write(".");
            serial_write_u64(subclass as u64, 2); serial_write(") vendor=");
            serial_write_u64(vid as u64, 4); serial_write("\n");
        }
    }

    ctx.pci_count = count;
    serial_write("[pci] Found ");
    serial_write_u64(count as u64, 10);
    serial_write(" devices\n");
}

// ═══════════════════════════════════════════════════════════════════════════
// LAPIC — Local APIC init + calibration via PIT
// ═══════════════════════════════════════════════════════════════════════════

const IA32_APIC_BASE: u32 = 0x1B;
const APIC_SIVR: u64 = 0x0F0;
const APIC_TPR: u64 = 0x080;
const APIC_LVT_TIMER: u64 = 0x320;
const APIC_TIMER_DCR: u64 = 0x3E0;
const APIC_TIMER_ICR: u64 = 0x380;
const APIC_TIMER_CCR: u64 = 0x390;
const APIC_EOI: u64 = 0x0B0;

fn lapic_read(reg: u64) -> u32 {
    unsafe { (phys_to_virt(0xFEE00000 + reg) as *const u32).read_volatile() }
}

fn lapic_write(reg: u64, val: u32) {
    unsafe { (phys_to_virt(0xFEE00000 + reg) as *mut u32).write_volatile(val); }
}

fn pit_wait_10ms() {
    // Set PIT channel 2 (mode 0, LSB then MSB) for a 10ms delay
    // PIT runs at 1.193182 MHz → 11932 cycles ≈ 10ms
    unsafe {
        outb(0x61, inb(0x61) & !3);   // Disable speaker + gate
        outb(0x43, 0xB0);              // Channel 2, mode 0, LSB+MSB
        let count: u16 = 11932;
        outb(0x42, (count & 0xFF) as u8);
        outb(0x42, (count >> 8) as u8);
        // Enable gate
        outb(0x61, inb(0x61) | 1);
        // Wait for countdown to finish (OUT bit C2, port 0x61 bit 5)
        while inb(0x61) & 0x20 == 0 {}
    }
}

fn lapic_init() {
    // 1. Enable LAPIC via SIVR (spurious interrupt vector register)
    let sivr = lapic_read(APIC_SIVR) | 0x100; // bit 8 = enable
    lapic_write(APIC_SIVR, sivr | 0xFF);       // vector 255

    // 2. Set TPR = 0
    lapic_write(APIC_TPR, 0);

    // 3. Configure LVT timer: one-shot, unmasked, vector 48
    lapic_write(APIC_LVT_TIMER, 48);

    // 4. Divide configuration = 16
    lapic_write(APIC_TIMER_DCR, 3); // divide by 16

    // 5. Calibrate: write max count, wait 10ms via PIT, read elapsed
    let max_count: u32 = 0xFFFFFFFF;
    lapic_write(APIC_TIMER_ICR, max_count);
    pit_wait_10ms();
    let elapsed = max_count.wrapping_sub(lapic_read(APIC_TIMER_CCR));

    // 6. Calculate ticks per second: (elapsed * 100) Hz (since 10ms = 1/100 sec)
    let ticks_per_sec = elapsed * 100;
    serial_write("[lapic] Calibrated: ");
    serial_write_u64(ticks_per_sec as u64, 10);
    serial_write(" ticks/s\n");

    // 7. Set periodic mode with a reasonable tick rate (~1000 Hz)
    let tick_count = ticks_per_sec / 1000; // ~1ms per tick
    lapic_write(APIC_LVT_TIMER, 48 | (1 << 17)); // periodic
    lapic_write(APIC_TIMER_DCR, 3);              // divide by 16
    lapic_write(APIC_TIMER_ICR, tick_count);

    serial_write("[lapic] Timer initialized at ~1000 Hz\n");
}

// ═══════════════════════════════════════════════════════════════════════════
// I/O APIC — redirection table initialization
// ═══════════════════════════════════════════════════════════════════════════

fn ioapic_read(base: u64, reg: u32) -> u32 {
    unsafe {
        let ptr = phys_to_virt(base) as *mut u32;
        ptr.add(0).write_volatile(reg); // IOREGSEL
        ptr.add(4).read_volatile()      // IOWIN
    }
}

fn ioapic_write(base: u64, reg: u32, val: u32) {
    unsafe {
        let ptr = phys_to_virt(base) as *mut u32;
        ptr.add(0).write_volatile(reg);
        ptr.add(4).write_volatile(val);
    }
}

fn ioapic_init(base: u64) {
    let ver = ioapic_read(base, 1);
    let max_redir = (ver >> 16) & 0xFF;

    serial_write("[ioapic] Base 0x");
    serial_write_u64(base, 16);
    serial_write(" version 0x");
    serial_write_hex32(ver);
    serial_write("\n");

    // Mask all redirection entries
    for i in 0..=max_redir {
        ioapic_write(base, 0x10 + i * 2 + 1, 0x00010000); // mask (bit 16)
        ioapic_write(base, 0x10 + i * 2, 0);
    }
    serial_write("[ioapic] All IRQs masked\n");
}

// ═══════════════════════════════════════════════════════════════════════════
// HPET — High Precision Event Timer
// ═══════════════════════════════════════════════════════════════════════════

fn hpet_init(base: u64) {
    unsafe {
        let ptr = phys_to_virt(base) as *mut u64;
        let cap = ptr.read_volatile();
        let period_fs = (cap >> 32) & 0xFFFFFFFF;

        serial_write("[hpet] Base 0x");
        serial_write_u64(base, 16);
        serial_write(" period=");
        serial_write_u64(period_fs, 10);
        serial_write(" fs\n");

        // Enable: set bit 0 (enable) + bit 1 (legacy replacement)
        let config = ptr.add(1).read_volatile();
        ptr.add(1).write_volatile(config | 3);

        // Reset counter
        ptr.add(2).write_volatile(0);
    }
    serial_write("[hpet] Enabled\n");
}

// ═══════════════════════════════════════════════════════════════════════════
// i8042 PS/2 controller
// ═══════════════════════════════════════════════════════════════════════════

fn i8042_wait_write() {
    let mut timeout = 100_000u32;
    while timeout > 0 {
        if inb(0x64) & 2 == 0 { return; }
        timeout -= 1;
    }
}

fn i8042_wait_read() -> bool {
    let mut timeout = 100_000u32;
    while timeout > 0 {
        if inb(0x64) & 1 != 0 { return true; }
        timeout -= 1;
    }
    false
}

fn i8042_command(cmd: u8) -> bool {
    i8042_wait_write();
    outb(0x64, cmd);
    true
}

fn i8042_write_data(val: u8) {
    i8042_wait_write();
    outb(0x60, val);
}

fn i8042_read_data() -> u8 {
    i8042_wait_read();
    inb(0x60)
}

fn i8042_read_timeout() -> Option<u8> {
    if inb(0x64) & 1 != 0 { Some(inb(0x60)) } else { None }
}

fn i8042_flush() {
    while i8042_read_timeout().is_some() {}
}

fn i8042_init() -> bool {
    i8042_flush();

    // Disable both ports
    i8042_command(0xAD);
    i8042_command(0xA7);

    // Read config byte
    i8042_command(0x20);
    let config = i8042_read_data();
    let mut config = config & !0x33; // clear IRQ1, IRQ12 enable + clock disable

    // Write config
    i8042_wait_write();
    outb(0x64, 0x60);
    i8042_write_data(config);

    // Test keyboard port
    i8042_command(0xAB);
    if i8042_read_data() != 0x00 {
        serial_write("[i8042] Keyboard port test failed\n");
        return false;
    }

    // Enable keyboard port
    i8042_command(0xAE);
    config |= 1; // enable IRQ1
    i8042_wait_write();
    outb(0x64, 0x60);
    i8042_write_data(config);

    // Enable keyboard scanning
    i8042_write_data(0xF4);
    if i8042_read_data() != 0xFA {
        serial_write("[i8042] Keyboard enable failed\n");
    }

    // Test mouse port
    i8042_command(0xA9);
    if i8042_read_data() != 0x00 {
        serial_write("[i8042] Mouse port not present\n");
    } else {
        // Enable mouse port
        i8042_command(0xA8);
        config |= 2; // enable IRQ12
        i8042_wait_write();
        outb(0x64, 0x60);
        i8042_write_data(config);

        // Enable mouse data reporting
        i8042_write_data(0xF4);
        i8042_read_data(); // ack
        serial_write("[i8042] Mouse enabled\n");
    }

    serial_write("[i8042] PS/2 controller initialized\n");
    true
}

fn i8042_poll() {
    while let Some(byte) = i8042_read_timeout() {
        // Route to keyboard or mouse based on status bit 5
        if inb(0x64) & 0x20 != 0 {
            // Mouse byte (not fully parsed in stage3)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Entry Point
// ═══════════════════════════════════════════════════════════════════════════

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut BootContext) -> ! {
    let ctx = unsafe { &mut *ctx_ptr };
    serial_write("\n[stage3] Device init — ACPI, PCI, APIC, HPET, i8042\n");

    // ── 1. ACPI parsing ──────────────────────────────────
    let acpi_info = acpi_init(ctx);

    // ── 2. PCI enumeration ───────────────────────────────
    pci_enumerate(ctx, &acpi_info);

    // ── 3. I/O APIC ──────────────────────────────────────
    let ioapic_base = acpi_info.ioapic_base.unwrap_or(0xFEC00000);
    if i8042_read_test(ioapic_base) {
        ctx.ioapic_base = ioapic_base;
        ioapic_init(ioapic_base);
    }
    fn i8042_read_test(base: u64) -> bool {
        let id = ioapic_read(base, 0);
        id != 0 && id != 0xFFFFFFFF
    }

    // ── 4. LAPIC init + timer calibration ────────────────
    lapic_init();

    // ── 5. HPET init ─────────────────────────────────────
    if let Some(hpet_base) = acpi_info.hpet_base {
        ctx.hpet_base = hpet_base;
        hpet_init(hpet_base);
    } else {
        serial_write("[hpet] Not found via ACPI, trying 0xFED00000\n");
        let hpet = 0xFED00000u64;
        let cap = unsafe { (phys_to_virt(hpet) as *const u64).read_volatile() };
        if cap != 0 && cap != 0xFFFFFFFFFFFFFFFF {
            ctx.hpet_base = hpet;
            hpet_init(hpet);
        }
    }

    // ── 6. PS/2 controller ───────────────────────────────
    i8042_init();

    // ── 7. Store ACPI info ──────────────────────────────
    if acpi_info.rsdp != 0 { ctx.rsdp = acpi_info.rsdp; }

    serial_write("[stage3] Device init complete\n");
    serial_write("[stage3] Context updated, jumping to kernel\n");

    // ── Jump to Kernel ──────────────────────────────────
    let kernel_entry = ctx.stage_entry[3];
    if kernel_entry != 0 {
        unsafe {
            let kernel_fn: extern "C" fn(*mut BootContext) -> ! =
                core::mem::transmute(kernel_entry);
            kernel_fn(ctx_ptr);
        }
    }

    loop { unsafe { asm!("hlt"); } }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { asm!("hlt"); } }
}
