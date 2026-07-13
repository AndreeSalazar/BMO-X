//! Faggin stage 12 ??? Devices: ACPI tables + PCI + LAPIC + I/O APIC + HPET + i8042.
//!
//! Responsibilities (one only):
//!   - Parse MCFG, HPET, MADT, FADT from the RSDP-found XSDT.
//!   - Enumerate PCI bus (ECAM if MCFG, else legacy IO).
//!   - LAPIC: enable + calibrate via PIT.
//!   - I/O APIC: mask all entries.
//!   - HPET: enable + reset counter.
//!   - i8042 PS/2: enable keyboard and mouse.
//!   - Publish ioapic_base, hpet_base, pci_count, pci_devices[].
//!   - Jump to kernel@0x400000 (ctx.stage_entry[0]).

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;
use boot_context::PciDevice;

const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;
const COM1: u16 = 0x3F8;

#[inline]
fn phys_to_virt(p: u64) -> u64 { p + HIGH_MEM_BASE }

#[inline]
fn outb(port: u16, val: u8) { unsafe { asm!("out dx, al", in("dx") port, in("al") val); } }
#[inline]
fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe { asm!("in al, dx", in("dx") port, out("al") v); }
    v
}
#[inline]
fn inl(port: u16) -> u32 {
    let v: u32;
    unsafe { asm!("in eax, dx", in("dx") port, out("eax") v); }
    v
}
#[inline]
fn outl(port: u16, val: u32) { unsafe { asm!("out dx, eax", in("dx") port, in("eax") val); } }

unsafe fn serial_puts(s: &str) {
    for &b in s.as_bytes() {
        if b == b'\n' {
            outb(COM1, b'\r');
        }
        outb(COM1, b);
    }
}

unsafe fn serial_hex(mut v: u64) {
    if v == 0 { outb(COM1, b'0'); return; }
    let mut buf = [0u8; 16];
    let mut i = 0;
    while v > 0 {
        buf[i] = b"0123456789abcdef"[(v & 0xF) as usize];
        v >>= 4; i += 1;
    }
    while i > 0 { i -= 1; outb(COM1, buf[i]); }
}

unsafe fn serial_dec(mut v: usize) {
    if v == 0 { outb(COM1, b'0'); return; }
    let mut buf = [0u8; 20];
    let mut i = 0;
    while v > 0 { buf[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
    while i > 0 { i -= 1; outb(COM1, buf[i]); }
}

// ?????? ACPI tables (minimal) ?????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

const RSDP_SIG: [u8; 8] = *b"RSD PTR ";
const MCFG_SIG: [u8; 4] = *b"MCFG";
const HPET_SIG: [u8; 4] = *b"HPET";
const MADT_SIG: [u8; 4] = *b"APIC";

#[repr(C, packed)]
struct Sdt { sig: [u8; 4], len: u32, rev: u8, sum: u8, _oem: [u8; 6], _oem_tid: [u8; 8], _oem_rev: u32, _cid: [u8; 4], _crev: u32 }

unsafe fn acpi_sum(p: *const u8, len: usize) -> u8 {
    let mut s: u8 = 0;
    for i in 0..len { s = s.wrapping_add(p.add(i).read()); }
    s
}

unsafe fn xsdt_base(rsdp: u64) -> u64 {
    let rev = (rsdp as *const u8).add(15).read();
    if rev >= 2 {
        let p = rsdp as *const u64;
        if (rsdp as *const u8).add(20).read() >= 24 {
            return p.add(3).read();
        }
        0
    } else {
        (rsdp as *const u32).add(2).read() as u64
    }
}

unsafe fn find_table(xsdt: u64, sig: &[u8; 4]) -> Option<u64> {
    if xsdt == 0 { return None; }
    let h = xsdt as *const Sdt;
    let entries = ((*h).len as usize - core::mem::size_of::<Sdt>()) / 8;
    for i in 0..entries {
        let e = (xsdt + core::mem::size_of::<Sdt>() as u64 + (i * 8) as u64) as *const u64;
        let addr = e.read();
        let t = addr as *const Sdt;
        if &(*t).sig == sig { return Some(addr); }
    }
    None
}

struct Devs { mcfg: Option<u64>, mcfg_end: Option<u8>, hpet: Option<u64>, lapic: Option<u64>, ioapic: Option<u64> }

unsafe fn parse_acpi(rsdp: u64) -> Devs {
    let mut d = Devs { mcfg: None, mcfg_end: None, hpet: None, lapic: None, ioapic: None };
    let xsdt = xsdt_base(rsdp);
    if let Some(addr) = find_table(xsdt, &MCFG_SIG) {
        let base = (addr + 44) as *const u64;
        let bus_end = (addr + 52) as *const u8;
        d.mcfg = Some(base.read());
        d.mcfg_end = Some(bus_end.read());
        serial_puts("[s12 acpi] MCFG ECAM 0x"); serial_hex(d.mcfg.unwrap()); serial_puts("\n");
    }
    if let Some(addr) = find_table(xsdt, &HPET_SIG) {
        let base = (addr + 40 + 8) as *const u64;
        d.hpet = Some(base.read());
        serial_puts("[s12 acpi] HPET 0x"); serial_hex(d.hpet.unwrap()); serial_puts("\n");
    }
    if let Some(addr) = find_table(xsdt, &MADT_SIG) {
        let lapic = (addr + 36) as *const u32;
        d.lapic = Some(lapic.read() as u64);
        serial_puts("[s12 acpi] LAPIC 0x"); serial_hex(d.lapic.unwrap()); serial_puts("\n");
        // Walk MADT body for I/O APIC
        let len = (addr as *const Sdt).read().len as usize;
        let mut off = 44;
        while off + 2 <= len {
            let t = (addr + off as u64) as *const u8;
            let etype = t.read();
            let elen = t.add(1).read() as usize;
            if etype == 1 {
                let ioapic = (addr + off as u64 + 4) as *const u32;
                d.ioapic = Some(ioapic.read() as u64);
                serial_puts("[s12 acpi] I/O APIC 0x"); serial_hex(d.ioapic.unwrap()); serial_puts("\n");
            }
            off += elen;
        }
    }
    d
}

// ?????? PCI ???????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

const PCI_CFG_ADDR: u16 = 0xCF8;
const PCI_CFG_DATA: u16 = 0xCFC;

unsafe fn pci_read32_io(bus: u8, dev: u8, off: u8) -> u32 {
    let addr: u32 = 0x80000000 | ((bus as u32) << 16) | ((dev as u32) << 11) | (off as u32 & 0xFC);
    outl(PCI_CFG_ADDR, addr);
    inl(PCI_CFG_DATA)
}
unsafe fn pci_read32_ecam(base: u64, bus: u8, dev: u8, off: u8) -> u32 {
    let p = phys_to_virt(base + ((bus as u64) << 20) | ((dev as u64) << 15) | (off as u64));
    (p as *const u32).read_volatile()
}

fn pci_scan(ctx: &mut boot_context::BootContext, d: &Devs) {
    let ecam = d.mcfg;
    let max_bus = d.mcfg_end.unwrap_or(0xFF);
    let mut count: u32 = 0;
    for bus in 0..=max_bus as u16 {
        for dev in 0..32u8 {
            let r0 = unsafe {
                if let Some(b) = ecam { pci_read32_ecam(b, bus as u8, dev, 0) } else { pci_read32_io(bus as u8, dev, 0) }
            };
            if (r0 & 0xFFFF) == 0xFFFF { continue; }
            let r2 = unsafe {
                if let Some(b) = ecam { pci_read32_ecam(b, bus as u8, dev, 8) } else { pci_read32_io(bus as u8, dev, 8) }
            };
            let vid = r0 as u16;
            let did = (r0 >> 16) as u16;
            let class = ((r2 >> 24) & 0xFF) as u8;
            let sub = ((r2 >> 16) & 0xFF) as u8;
            if (count as usize) < ctx.pci_devices.len() {
                ctx.pci_devices[count as usize] = PciDevice {
                    bus: bus as u8, device: dev, function: 0,
                    class, subclass: sub, vendor_id: vid, device_id: did, bar0: 0,
                };
            }
            count += 1;
        }
    }
    ctx.pci_count = count;
    unsafe { serial_puts("[s12 pci] found "); serial_dec(count as usize); serial_puts(" devices\n"); }
}

// ?????? LAPIC ??????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

unsafe fn lapic_init() {
    let base = phys_to_virt(0xFEE00000);
    // SIVR enable + spurious vector 0xFF
    let sivr = (base as *mut u32).add(0x0F0 / 4).read_volatile() | 0x100;
    (base as *mut u32).add(0x0F0 / 4).write_volatile(sivr | 0xFF);
    (base as *mut u32).add(0x080 / 4).write_volatile(0); // TPR
    (base as *mut u32).add(0x320 / 4).write_volatile(48); // LVT timer one-shot
    (base as *mut u32).add(0x3E0 / 4).write_volatile(3);   // DCR divide by 16
    // PIT 10 ms wait
    outb(0x61, inb(0x61) & !3);
    outb(0x43, 0xB0);
    outb(0x42, (11932 & 0xFF) as u8);
    outb(0x42, (11932 >> 8) as u8);
    outb(0x61, inb(0x61) | 1);
    while inb(0x61) & 0x20 == 0 {}
    (base as *mut u32).add(0x380 / 4).write_volatile(0xFFFF_FFFF);
    let elapsed = 0xFFFF_FFFFu32.wrapping_sub((base as *mut u32).add(0x390 / 4).read_volatile());
    let hz = elapsed as u64 * 100;
    serial_puts("[s12 lapic] "); serial_dec(hz as usize); serial_puts(" ticks/s\n");
    (base as *mut u32).add(0x320 / 4).write_volatile(48 | (1 << 17));
    (base as *mut u32).add(0x380 / 4).write_volatile((hz / 1000) as u32);
}

// ?????? I/O APIC ?????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

unsafe fn ioapic_init(base: u64) {
    let v = phys_to_virt(base) as *mut u32;
    let ver = v.add(1).read_volatile();
    let max = (ver >> 16) & 0xFF;
    let max_usize = max as usize;
    for i in 0..=max_usize {
        v.add((0x10 / 4 + i * 2 + 1) as usize).write_volatile(0x00010000);
        v.add((0x10 / 4 + i * 2) as usize).write_volatile(0);
    }
    serial_puts("[s12 ioapic] all masked\n");
}

// ?????? HPET ?????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

unsafe fn hpet_init(base: u64) {
    let v = phys_to_virt(base) as *mut u64;
    let cap = v.read_volatile();
    let period = (cap >> 32) & 0xFFFFFFFF;
    serial_puts("[s12 hpet] period "); serial_dec(period as usize); serial_puts(" fs\n");
    let cfg = v.add(1).read_volatile();
    v.add(1).write_volatile(cfg | 3);
    v.add(2).write_volatile(0);
}

// ?????? i8042 ??????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

unsafe fn i8042_init() {
    outb(0x64, 0xAD); outb(0x64, 0xA7);
    outb(0x64, 0x20); let cfg = inb(0x60);
    outb(0x64, 0x60); outb(0x60, cfg & !0x33);
    outb(0x64, 0xAB); let _ = inb(0x60);
    outb(0x64, 0xAE); outb(0x60, 0xF4); let _ = inb(0x60);
    outb(0x64, 0xA9);
    if inb(0x60) == 0 {
        outb(0x64, 0xA8);
        outb(0x64, 0x60); outb(0x60, cfg | 2);
        outb(0x60, 0xF4); let _ = inb(0x60);
    }
    serial_puts("[s12 i8042] PS/2 controller ready\n");
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    unsafe { serial_puts("\n[s12 devices]\n"); }

    let rsdp = unsafe { (*ctx_ptr).rsdp };
    let devs = if rsdp != 0 { unsafe { parse_acpi(rsdp) } } else {
        Devs { mcfg: None, mcfg_end: Some(0xFF), hpet: None, lapic: Some(0xFEE00000), ioapic: Some(0xFEC00000) }
    };

    unsafe { serial_puts("\n[s12 pci]\n"); }
    pci_scan(unsafe { &mut *ctx_ptr }, &devs);

    if let Some(base) = devs.ioapic { unsafe { ioapic_init(base); } }
    unsafe { lapic_init(); }
    if let Some(base) = devs.hpet { unsafe { hpet_init(base); } }
    else { unsafe {
        // Try default 0xFED00000
        let v = phys_to_virt(0xFED00000) as *const u64;
        if v.read_volatile() != 0 && v.read_volatile() != !0u64 { hpet_init(0xFED00000); }
    } }
    unsafe { i8042_init(); }

    let ctx = unsafe { &mut *ctx_ptr };
    ctx.ioapic_base = devs.ioapic.unwrap_or(0);
    ctx.hpet_base   = devs.hpet.unwrap_or(0);
    ctx.rsdp = rsdp;

    unsafe { serial_puts("[s12] -> jmp kernel\n"); }
    let entry = ctx.stage_entry[0];
    unsafe {
        asm!(
            "jmp {next}",
            next = in(reg) entry,
            in("rdi") ctx_ptr,
            options(noreturn)
        );
    }
}

// Symbol that the next-stage binary references.
#[no_mangle]
pub extern "C" fn kernel_entry(ctx: *mut boot_context::BootContext) -> ! {
    loop { unsafe { asm!("hlt"); } }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
