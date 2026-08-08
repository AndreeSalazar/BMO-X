//! s2_mem -- Memory + device initialization stage.
//!
//! Combines: PML4 page tables, heap bitmap, ACPI RSDP scan,
//! PCI bus scan, LAPIC/IOAPIC/HPET/i8042 init.
//!
//! CRITICAL SEQUENCE:
//!   1. Build PML4 (identity 0..32 MiB + higher-half 0..16 GiB)
//!   2. Map BootContext page + GOP framebuffer
//!   3. Write ctx.pml4 BEFORE CR3 switch
//!   4. Switch to safe stack in BSS
//!   5. mov cr3 + jmp kernel (ATOMIC -- no stages in between)
//!   6. After jmp: kernel does BSS zero + kernel_main_real
//!
//! Device init happens AFTER the CR3 switch but BEFORE jumping to
//! kernel, using the higher-half mapping for MMIO access.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]
#![allow(unsafe_op_in_unsafe_fn)]

use core::panic::PanicInfo;
use core::arch::{asm, naked_asm};
use boot_context::PciDevice;

const KERNEL_ADDR: u64 = 0x400000;

const PAGE_SIZE: u64 = 4096;
const HIGH_MEM_BASE: u64 = 0xFFFF_8000_0000_0000;

const PTE_PRESENT:    u64 = 1 << 0;
const PTE_WRITABLE:   u64 = 1 << 1;
const PTE_HUGE:       u64 = 1 << 7;
const PTE_GLOBAL:     u64 = 1 << 8;
const PTE_CACHE_DISABLE: u64 = 1 << 4;

const fn pte_addr(e: u64) -> u64 { e & 0x000F_FFFF_FFFF_F000 }

// -- Safe stack in .bss (within identity-mapped region) ------------

// The 64 KiB transition stack is reserved by linker.ld.

// -- Frame pool (page table frames) -------------------------------

const POOL_SIZE: usize = 64;
static mut POOL: [u64; POOL_SIZE / 64] = [0u64; POOL_SIZE / 64];
static mut POOL_BASE: u64 = 0;
static mut POOL_END: u64 = 0;

unsafe fn pool_init(ctx: &mut boot_context::BootContext) {
    // 32 MiB = 0x2000000 (covers s1_cpu + s2_mem + kernel + room)
    const REGION_END: u64 = 0x2000000;
    for e in &mut ctx.memory_map[..ctx.memory_map_count as usize] {
        if e.kind != 1 || e.size == 0 { continue; }
        let entry_end = e.base + e.size;
        if entry_end <= REGION_END { continue; }
        let base = if e.base < REGION_END {
            REGION_END
        } else {
            (e.base + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
        };
        let pool_end = base + (POOL_SIZE as u64) * PAGE_SIZE;
        if pool_end > entry_end { continue; }
        POOL_BASE = base;
        POOL_END = pool_end;
        // Remove page-table frames from the map consumed by the kernel.
        e.base = pool_end;
        e.size = entry_end - pool_end;
        serial_shared::puts("[s2_mem] pool base=0x");
        serial_shared::hex(base);
        serial_shared::puts("\n");
        break;
    }
    if POOL_BASE == 0 {
        serial_shared::puts("[s2_mem] FATAL: no pool memory\n");
        loop { asm!("hlt"); }
    }
}

unsafe fn pool_alloc() -> *mut u64 {
    for i in 0..POOL_SIZE {
        if POOL[i / 64] & (1 << (i % 64)) == 0 {
            POOL[i / 64] |= 1 << (i % 64);
            return (POOL_BASE + (i as u64) * PAGE_SIZE) as *mut u64;
        }
    }
    core::ptr::null_mut()
}

unsafe fn zeroed_frame() -> &'static mut [u64; 512] {
    let p = pool_alloc() as *mut [u64; 512];
    if p.is_null() {
        serial_shared::puts("[s2_mem] FATAL: out of frames\n");
        loop { asm!("hlt"); }
    }
    core::ptr::write_bytes(p as *mut u8, 0, PAGE_SIZE as usize);
    &mut *p
}

unsafe fn get_or_create(table: *mut u64, idx: usize) -> *mut u64 {
    let entry = table.add(idx).read_volatile();
    if entry & PTE_PRESENT == 0 {
        let p = zeroed_frame();
        table.add(idx).write_volatile(p.as_ptr() as u64 | PTE_PRESENT | PTE_WRITABLE);
        return p.as_mut_ptr();
    }
    // If this is a huge page entry (PS bit set), we can't descend further.
    // This should not happen for our mapping strategy.
    if entry & PTE_HUGE != 0 {
        serial_shared::puts("[s2_mem] FATAL: huge page collision\n");
        loop { asm!("hlt"); }
    }
    (entry & pte_addr(!0u64)) as *mut u64
}

unsafe fn map_page(pml4: *mut u64, v: u64, p: u64, flags: u64) {
    let i4 = ((v >> 39) & 0x1FF) as usize;
    let i3 = ((v >> 30) & 0x1FF) as usize;
    let i2 = ((v >> 21) & 0x1FF) as usize;
    let i1 = ((v >> 12) & 0x1FF) as usize;

    let pdpt = get_or_create(pml4, i4);
    let pd   = get_or_create(pdpt, i3);
    let pt   = get_or_create(pd,   i2);

    let entry = (p & pte_addr(!0u64)) | flags;
    pt.add(i1).write_volatile(entry);
}

unsafe fn map_2m_huge(pml4: *mut u64, v_start: u64, p_start: u64, count_2m: usize, flags: u64) {
    for i in 0..count_2m {
        let v = v_start + (i as u64) * 0x20_0000u64;
        let p = p_start + (i as u64) * 0x20_0000u64;
        let i4 = ((v >> 39) & 0x1FF) as usize;
        let i3 = ((v >> 30) & 0x1FF) as usize;
        let i2 = ((v >> 21) & 0x1FF) as usize;

        let pdpt = get_or_create(pml4, i4);
        let pd   = get_or_create(pdpt, i3);
        let entry = (p & pte_addr(!0u64)) | flags | PTE_HUGE;
        pd.add(i2).write_volatile(entry);
    }
}

// -- Heap bitmap ---------------------------------------------------

const MAX_FRAMES: usize = 32768; // 128 MB
static mut FRAME_BITMAP: [u64; MAX_FRAMES / 64] = [0u64; MAX_FRAMES / 64];
static mut FRAME_BASE: u64 = 0;
static mut FRAME_COUNT: u64 = 0;

unsafe fn heap_init(ctx: &boot_context::BootContext) {
    for entry in &ctx.memory_map[..ctx.memory_map_count as usize] {
        if entry.kind != 1 || entry.size == 0 { continue; }
        let base = (entry.base + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let end  = entry.base + entry.size;
        if end <= base { continue; }
        FRAME_BASE = base;
        FRAME_COUNT = (end - base) / PAGE_SIZE;
        break;
    }
    // Mark first 32 MiB as used (stages + kernel)
    for i in 0..(0x2000000u64 / PAGE_SIZE) as usize {
        if i < MAX_FRAMES {
            FRAME_BITMAP[i / 64] |= 1 << (i % 64);
        }
    }
    serial_shared::puts("[s2_mem] heap: ");
    serial_shared::dec(FRAME_COUNT as usize);
    serial_shared::puts(" frames\n");
}

// -- ACPI ----------------------------------------------------------

const RSDP_SIG: [u8; 8] = *b"RSD PTR ";
const MCFG_SIG: [u8; 4] = *b"MCFG";
const HPET_SIG: [u8; 4] = *b"HPET";
const MADT_SIG: [u8; 4] = *b"APIC";

#[repr(C, packed)]
struct Sdt { sig: [u8; 4], len: u32, rev: u8, sum: u8, _oem: [u8; 6], _oem_tid: [u8; 8], _oem_rev: u32, _cid: [u8; 4], _crev: u32 }

fn checksum_ok(addr: u64, len: usize) -> bool {
    let ptr = addr as *const u8;
    let mut sum: u8 = 0;
    for i in 0..len {
        sum = sum.wrapping_add(unsafe { ptr.add(i).read() });
    }
    sum == 0
}

fn matches_rsdp(addr: u64) -> bool {
    let ptr = addr as *const u8;
    for i in 0..8 {
        if unsafe { ptr.add(i).read() } != RSDP_SIG[i] { return false; }
    }
    let rev = unsafe { ptr.add(15).read() };
    let len: usize = if rev >= 2 { unsafe { ptr.add(20).read() as usize } } else { 20 };
    checksum_ok(addr, len)
}

fn scan_rsdp() -> u64 {
    let ebda_seg: u16 = unsafe { (0x40E as *const u16).read() };
    let ebda_start = (ebda_seg as u64) << 4;
    let mut a = ebda_start;
    while a < ebda_start + 1024 {
        if matches_rsdp(a) { return a; }
        a += 16;
    }
    let mut a = 0xE0000u64;
    while a < 0x100000 {
        if matches_rsdp(a) { return a; }
        a += 16;
    }
    0
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

struct AcpiInfo {
    mcfg: Option<u64>,
    mcfg_end: Option<u8>,
    hpet: Option<u64>,
    lapic: Option<u64>,
    ioapic: Option<u64>,
}

unsafe fn acpi_parse(rsdp: u64) -> AcpiInfo {
    let mut info = AcpiInfo { mcfg: None, mcfg_end: None, hpet: None, lapic: None, ioapic: None };
    if rsdp == 0 {
        serial_shared::puts("[s2_mem] RSDP not found\n");
        return info;
    }
    serial_shared::puts("[s2_mem] RSDP at 0x");
    serial_shared::hex(rsdp);
    serial_shared::puts("\n");

    let xsdt = xsdt_base(rsdp);
    if let Some(addr) = find_table(xsdt, &MCFG_SIG) {
        let base = (addr + 44) as *const u64;
        let bus_end = (addr + 52) as *const u8;
        info.mcfg = Some(base.read());
        info.mcfg_end = Some(bus_end.read());
        serial_shared::puts("[s2_mem] MCFG ECAM 0x");
        serial_shared::hex(info.mcfg.unwrap());
        serial_shared::puts("\n");
    }
    if let Some(addr) = find_table(xsdt, &HPET_SIG) {
        // HPET table: 36-byte SDT header, then Event Timer Block ID (4B,
        // offset 36), then a Generic Address Structure at offset 40 whose
        // 8-byte Address field is at offset 44 (40 + 4B GAS header), NOT
        // offset 48. The old +8 read garbage -> a bogus base that page-
        // faulted in hpet_init on real hardware.
        let base = (addr + 44) as *const u64;
        info.hpet = Some(base.read());
        serial_shared::puts("[s2_mem] HPET 0x");
        serial_shared::hex(info.hpet.unwrap());
        serial_shared::puts("\n");
    }
    if let Some(addr) = find_table(xsdt, &MADT_SIG) {
        let lapic = (addr + 36) as *const u32;
        info.lapic = Some(lapic.read() as u64);
        serial_shared::puts("[s2_mem] LAPIC 0x");
        serial_shared::hex(info.lapic.unwrap());
        serial_shared::puts("\n");
        let len = (addr as *const Sdt).read().len as usize;
        let mut off = 44;
        while off + 2 <= len {
            let t = (addr + off as u64) as *const u8;
            let etype = t.read();
            let elen = t.add(1).read() as usize;
            if etype == 1 {
                let ioapic = (addr + off as u64 + 4) as *const u32;
                info.ioapic = Some(ioapic.read() as u64);
                serial_shared::puts("[s2_mem] I/O APIC 0x");
                serial_shared::hex(info.ioapic.unwrap());
                serial_shared::puts("\n");
            }
            off += elen;
        }
    }
    info
}

// -- PCI -----------------------------------------------------------

const PCI_CFG_ADDR: u16 = 0xCF8;
const PCI_CFG_DATA: u16 = 0xCFC;

#[inline] fn phys_to_virt(p: u64) -> u64 { p + HIGH_MEM_BASE }
#[inline] fn outb(port: u16, val: u8) { unsafe { asm!("out dx, al", in("dx") port, in("al") val); } }
#[inline] fn inb(port: u16) -> u8 { let v: u8; unsafe { asm!("in al, dx", in("dx") port, out("al") v); } v }
#[inline] fn inl(port: u16) -> u32 { let v: u32; unsafe { asm!("in eax, dx", in("dx") port, out("eax") v); } v }
#[inline] fn outl(port: u16, val: u32) { unsafe { asm!("out dx, eax", in("dx") port, in("eax") val); } }

unsafe fn pci_read32_io(bus: u8, dev: u8, off: u8) -> u32 {
    let addr: u32 = 0x80000000 | ((bus as u32) << 16) | ((dev as u32) << 11) | (off as u32 & 0xFC);
    outl(PCI_CFG_ADDR, addr);
    inl(PCI_CFG_DATA)
}

unsafe fn pci_read32_ecam(base: u64, bus: u8, dev: u8, off: u8) -> u32 {
    let p = phys_to_virt(base + ((bus as u64) << 20) | ((dev as u64) << 15) | (off as u64));
    (p as *const u32).read_volatile()
}

fn pci_scan(ctx: &mut boot_context::BootContext, acpi: &AcpiInfo) {
    let ecam = acpi.mcfg;
    let max_bus = acpi.mcfg_end.unwrap_or(0xFF);
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
    serial_shared::puts("[s2_mem] PCI: ");
    serial_shared::dec(count as usize);
    serial_shared::puts(" devices\n");
}

// -- LAPIC ---------------------------------------------------------

fn pic_mask_all() {
    // BMO uses LAPIC/IOAPIC. Keep the legacy 8259 from delivering stale
    // firmware IRQs when Ring 0 eventually sets IF.
    outb(0x21, 0xFF);
    outb(0xA1, 0xFF);
}

#[inline]
fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack)); }
    ((high as u64) << 32) | low as u64
}

unsafe fn lapic_init(tsc_freq: u64) {
    let base = phys_to_virt(0xFEE00000);
    let sivr = (base as *mut u32).add(0x0F0 / 4).read_volatile() | 0x100;
    (base as *mut u32).add(0x0F0 / 4).write_volatile(sivr | 0xFF);
    (base as *mut u32).add(0x080 / 4).write_volatile(0);
    (base as *mut u32).add(0x320 / 4).write_volatile(48);
    (base as *mut u32).add(0x3E0 / 4).write_volatile(3);
    outb(0x61, inb(0x61) & !3);
    outb(0x43, 0xB0);
    outb(0x42, (11932 & 0xFF) as u8);
    outb(0x42, (11932 >> 8) as u8);
    outb(0x61, inb(0x61) | 1);
    (base as *mut u32).add(0x380 / 4).write_volatile(0xFFFF_FFFF);
    let started = rdtsc();
    let timeout_ticks = (tsc_freq / 50).max(10_000_000);
    while inb(0x61) & 0x20 == 0 {
        if rdtsc().wrapping_sub(started) >= timeout_ticks {
            (base as *mut u32).add(0x320 / 4).write_volatile(1 << 16);
            serial_shared::puts("[s2_mem] LAPIC calibration timeout; timer masked\n");
            return;
        }
        core::hint::spin_loop();
    }
    let elapsed = 0xFFFF_FFFFu32.wrapping_sub((base as *mut u32).add(0x390 / 4).read_volatile());
    let hz = elapsed as u64 * 100;
    serial_shared::puts("[s2_mem] LAPIC ");
    serial_shared::dec(hz as usize);
    serial_shared::puts(" ticks/s\n");
    (base as *mut u32).add(0x320 / 4).write_volatile(48 | (1 << 17));
    (base as *mut u32).add(0x380 / 4).write_volatile((hz / 1000) as u32);
}

// -- I/O APIC ------------------------------------------------------

unsafe fn ioapic_init(base: u64) {
    let select = phys_to_virt(base) as *mut u32;
    let window = (phys_to_virt(base) + 0x10) as *mut u32;
    select.write_volatile(1);
    let ver = window.read_volatile();
    let max = (ver >> 16) & 0xFF;
    for i in 0..=max {
        let low = 0x10 + i * 2;
        select.write_volatile(low + 1);
        window.write_volatile(0);
        select.write_volatile(low);
        window.write_volatile(1 << 16);
    }
    serial_shared::puts("[s2_mem] I/O APIC all masked\n");
}

// -- HPET ----------------------------------------------------------

unsafe fn hpet_init(base: u64) {
    let v = phys_to_virt(base) as *mut u64;
    let cap = v.read_volatile();
    let period = (cap >> 32) & 0xFFFFFFFF;
    serial_shared::puts("[s2_mem] HPET period ");
    serial_shared::dec(period as usize);
    serial_shared::puts(" fs\n");
    // Register offsets (as u64 index): General Config = 0x010 = index 2,
    // General Interrupt Status = 0x020 = index 4. (Was index 1/2 -- the
    // 0x008 slot is reserved.)
    let cfg = v.add(2).read_volatile();
    // Enable the main counter but do not claim legacy PIT/RTC routing.
    v.add(2).write_volatile((cfg | 1) & !2);
    v.add(4).write_volatile(0);
}

// -- i8042 PS/2 ----------------------------------------------------

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
    serial_shared::puts("[s2_mem] i8042 PS/2 ready\n");
}

// -- Entry point ---------------------------------------------------

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "mov r12, rdi",
        "lea rsp, [rip + {stack_end}]",
        "lea rax, [rip + __bss_start]",
        "lea rcx, [rip + __bss_end]",
        "sub rcx, rax",
        "mov rdi, rax",
        "xor eax, eax",
        "rep stosb",
        "mov rdi, r12",
        "call s2_main",
        "2: hlt",
        "jmp 2b",
        stack_end = sym S2_STACK_END,
    );
}

unsafe extern "C" {
    static S2_STACK_END: u8;
}

/// Boot-progress bar on the framebuffer (8 rows tall at `y0`). Visible
/// without a serial cable; only meaningful once s1 populated ctx.fb_addr
/// and the region is mapped (identity pre-CR3, and re-mapped after).
fn s2_bar(ctx: &boot_context::BootContext, y0: u64, color: u32) {
    if ctx.fb_addr == 0 { return; }
    let fb = ctx.fb_addr as *mut u32;
    for y in y0..y0 + 8 {
        for x in 0..ctx.fb_width as u64 {
            unsafe { fb.add((y * ctx.fb_stride as u64 + x) as usize).write_volatile(color); }
        }
    }
}

#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn s2_main(ctx_ptr: *mut boot_context::BootContext) -> ! {
    serial_shared::puts("\n[s2_mem] === MEMORY + DEVICES INIT ===\n");

    let ctx = unsafe { &mut *ctx_ptr };

    // Visual bisect marker: CYAN bar rows 8..16 = "s2 alive" (s1 painted
    // green above; the kernel paints magenta below).
    if ctx.fb_addr != 0 {
        let fb = ctx.fb_addr as *mut u32;
        for y in 8..16u64 {
            for x in 0..ctx.fb_width as u64 {
                unsafe { fb.add((y * ctx.fb_stride as u64 + x) as usize).write_volatile(0xFF00_BBDD); }
            }
        }
    }

    // 1. Init frame pool from memory map
    unsafe { pool_init(ctx); }

    // 2. Allocate PML4
    let pml4 = unsafe { zeroed_frame() };
    let pml4_phys = pml4.as_ptr() as u64;
    serial_shared::puts("[s2_mem] PML4 at 0x");
    serial_shared::hex(pml4_phys);
    serial_shared::puts("\n");

    // 3. Identity-map 0..4 GiB (2048 x 2 MiB huge pages).
    //    ACPI tables, PCI ECAM and device MMIO all live at physical
    //    addresses above 32 MiB (typically just under 4 GiB), and s2
    //    reads them by raw physical address. Mapping only 32 MiB made
    //    acpi_parse page-fault on real hardware. 4 GiB covers the whole
    //    low physical space the boot path touches.
    unsafe {
        map_2m_huge(pml4.as_mut_ptr(), 0x0, 0x0, 2048,
            PTE_PRESENT | PTE_WRITABLE);
    }
    serial_shared::puts("[s2_mem] identity-mapped 0..4GB\n");

    // 4. Higher-half mirror: 0..16 GiB -> 0xFFFF_8000_0000_0000
    unsafe {
        map_2m_huge(pml4.as_mut_ptr(), HIGH_MEM_BASE, 0x0, 8192,
            PTE_PRESENT | PTE_WRITABLE | PTE_GLOBAL);
    }
    serial_shared::puts("[s2_mem] higher-half 0..16GB\n");

    // 5. Map the GOP framebuffer as cache-disabled 2 MiB huge pages.
    //    Huge pages (not 4 KiB) so this never collides with the 0..4 GiB
    //    huge-page identity map above: for a framebuffer below 4 GiB this
    //    OVERRIDES those entries to cache-disable (MMIO writes must reach
    //    the device, not sit in a write-back cache); above 4 GiB it just
    //    adds the mapping.
    if ctx.fb_addr != 0 {
        const HUGE: u64 = 0x20_0000;
        let fb_2m_start = ctx.fb_addr & !(HUGE - 1);
        let fb_size = (ctx.fb_stride as u64) * (ctx.fb_height as u64) * 4;
        let fb_2m_pages = ((ctx.fb_addr + fb_size - fb_2m_start + HUGE - 1) / HUGE) as usize;
        unsafe {
            map_2m_huge(pml4.as_mut_ptr(), fb_2m_start, fb_2m_start, fb_2m_pages,
                PTE_PRESENT | PTE_WRITABLE | PTE_CACHE_DISABLE);
        }
        serial_shared::puts("[s2_mem] GOP fb mapped (");
        serial_shared::dec(fb_2m_pages);
        serial_shared::puts(" x 2MB)\n");
    }

    // 6. Map BootContext page if outside identity region
    let ctx_phys = ctx_ptr as u64;
    let ctx_page = ctx_phys & !(PAGE_SIZE - 1);
    if ctx_page >= 0x2000000 {
        unsafe {
            map_page(pml4.as_mut_ptr(), ctx_page, ctx_page,
                PTE_PRESENT | PTE_WRITABLE);
        }
        serial_shared::puts("[s2_mem] mapped BootContext at 0x");
        serial_shared::hex(ctx_page);
        serial_shared::puts("\n");
    }

    // 7. Write ctx.pml4 BEFORE CR3 switch
    ctx.pml4 = pml4_phys;

    // ===============================================================
    // 8. CRITICAL: Switch to safe stack + CR3 NOW.
    //
    //    After this point, the higher-half mapping is active, so
    //    phys_to_virt() works for MMIO access (LAPIC, IOAPIC, HPET).
    //    The identity map (0..32 MiB) keeps our code accessible.
    //    The safe stack is in .BSS at 0x200000 (identity-mapped).
    // ===============================================================
    serial_shared::puts("[s2_mem] switching CR3 → 0x");
    serial_shared::hex(pml4_phys);
    serial_shared::puts("\n");

    // _start already moved execution to the internal identity-mapped stack.
    unsafe {
        asm!("mov cr3, {}", in(reg) pml4_phys, options(nostack));
    }

    // ===============================================================
    //  NOW ON NEW PAGE TABLES. phys_to_virt() works.
    // ===============================================================
    serial_shared::puts("[s2_mem] CR3 switched OK\n");
    // YELLOW rows 16..24: paging switched + framebuffer re-mapped OK.
    s2_bar(ctx, 16, 0xFFFF_FF00);

    // 9. Heap init
    unsafe { heap_init(ctx); }

    // 10. ACPI parse
    let rsdp = scan_rsdp();
    let acpi = unsafe { acpi_parse(rsdp) };
    // ORANGE rows 24..32: heap + ACPI parse survived.
    s2_bar(ctx, 24, 0xFFFF_8800);

    // 11. PCI scan (ECAM uses phys_to_virt)
    pci_scan(ctx, &acpi);
    // RED rows 32..40: PCI scan survived.
    s2_bar(ctx, 32, 0xFFFF_0000);

    // 12. Store ACPI/device info in BootContext
    ctx.rsdp = rsdp;
    ctx.ioapic_base = acpi.ioapic.unwrap_or(0);
    ctx.hpet_base   = acpi.hpet.unwrap_or(0);

    // 13. Device init (LAPIC, IOAPIC, HPET, i8042)
    //     phys_to_virt() now works because higher-half is mapped.
    pic_mask_all();
    unsafe { lapic_init(ctx.tsc_freq); }
    s2_bar(ctx, 40, 0xFFFF_00FF); // rows 40..48 (pink): LAPIC done
    if let Some(ioapic_base) = acpi.ioapic {
        unsafe { ioapic_init(ioapic_base); }
    }
    s2_bar(ctx, 48, 0xFF00_FFFF); // rows 48..56 (aqua): IOAPIC done
    if let Some(hpet_base) = acpi.hpet {
        unsafe { hpet_init(hpet_base); }
    }
    s2_bar(ctx, 56, 0xFFFF_FF00); // rows 56..64 (yellow): HPET done
    unsafe { i8042_init(); }
    // WHITE rows 64..72: all device init done, about to jump to kernel.
    s2_bar(ctx, 64, 0xFFFF_FFFF);

    serial_shared::puts("[s2_mem] === ALL INIT DONE ===\n");
    serial_shared::puts("[s2_mem] jumping to kernel at 0x400000\n");

    // 14. Jump to kernel
    unsafe {
        asm!(
            "jmp {next}",
            next = in(reg) KERNEL_ADDR,
            in("rdi") ctx_ptr,
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
