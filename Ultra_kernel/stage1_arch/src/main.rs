#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![allow(dead_code)]

use core::panic::PanicInfo;
use core::arch::asm;
use boot_context::BootContext;

// ═══════════════════════════════════════════════════════════════════════════
// Serial I/O — COM1 (0x3F8) debug output
// ═══════════════════════════════════════════════════════════════════════════

const COM1: u16 = 0x3F8;

fn outb(port: u16, val: u8) {
    unsafe { asm!("out dx, al", in("dx") port, in("al") val); }
}

fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe { asm!("in al, dx", in("dx") port, out("al") v); }
    v
}

fn serial_init() {
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x80);
    outb(COM1 + 0, 0x01);
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03);
    outb(COM1 + 2, 0xC7);
    outb(COM1 + 4, 0x0B);
}

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

fn serial_write_u64_dec(value: u64) {
    if value == 0 { serial_write_byte(b'0'); return; }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut v = value;
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    while i < buf.len() { serial_write_byte(buf[i]); i += 1; }
}

fn serial_kv_u64(name: &str, val: u64) {
    serial_write(name);
    serial_write(" = 0x");
    serial_write_u64(val, 16);
    serial_write(" (");
    serial_write_u64_dec(val);
    serial_write(")\n");
}

// ═══════════════════════════════════════════════════════════════════════════
// GDT — Global Descriptor Table
// Layout: Null | Kernel CS | Kernel DS | User DS | User CS | TSS (16B)
// ═══════════════════════════════════════════════════════════════════════════

const KERNEL_CS: u16 = 0x08;
const KERNEL_DS: u16 = 0x10;
const USER_DS: u16 = 0x18 | 3;
const USER_CS: u16 = 0x20 | 3;
const TSS_SEL: u16 = 0x28;

#[repr(C, packed)]
struct Tss {
    _reserved0: u32,
    rsp: [u64; 3],
    _reserved1: u64,
    ist: [u64; 7],
    _reserved2: u64,
    _reserved3: u16,
    iomap_base: u16,
}

impl Tss {
    const fn new() -> Self {
        Self {
            _reserved0: 0,
            rsp: [0; 3],
            _reserved1: 0,
            ist: [0; 7],
            _reserved2: 0,
            _reserved3: 0,
            iomap_base: core::mem::size_of::<Tss>() as u16,
        }
    }
}

#[repr(C, align(16))]
struct Gdt {
    entries: [u64; 7],
}

#[repr(C, packed)]
struct Gdtr {
    limit: u16,
    base: u64,
}

#[repr(align(16))]
struct IstStack([u8; 8192]);

#[repr(align(16))]
struct KernelStack([u8; 16384]);

static mut TSS: Tss = Tss::new();
static mut GDT: Gdt = Gdt { entries: [0; 7] };
static mut IST1_STACK: IstStack = IstStack([0; 8192]);
static mut IST3_STACK: IstStack = IstStack([0; 8192]);
static mut KERNEL_STACK: KernelStack = KernelStack([0; 16384]);

fn make_segment(dpl: u8, code: bool) -> u64 {
    let mut d: u64 = 0;
    d |= 0xFFFF;
    d |= 0x0F << 48;
    let mut access: u8 = 0x92;
    access |= dpl << 5;
    if code { access |= 0x08; }
    d |= (access as u64) << 40;
    let mut flags: u8 = 0x0A;
    if !code { flags = 0x0C; }
    d |= (flags as u64) << 52;
    d
}

fn make_tss_descriptor(tss_addr: u64, tss_size: u16) -> (u64, u64) {
    let mut low: u64 = 0;
    low |= (tss_size as u64) & 0xFFFF;
    low |= (((tss_size as u64) >> 16) & 0x0F) << 48;
    low |= ((tss_addr & 0xFFFF) as u64) << 16;
    low |= (((tss_addr >> 16) & 0xFF) as u64) << 32;
    low |= (((tss_addr >> 24) & 0xFF) as u64) << 56;
    low |= 0x89u64 << 40;
    let high: u64 = (tss_addr >> 32) & 0xFFFFFFFF;
    (low, high)
}

unsafe fn init_gdt() {
    let stack_top = core::ptr::addr_of!(KERNEL_STACK) as u64 + 16384;
    TSS.rsp[0] = stack_top;
    let ist1_top = core::ptr::addr_of!(IST1_STACK) as u64 + 8192;
    TSS.ist[0] = ist1_top;
    let ist3_top = core::ptr::addr_of!(IST3_STACK) as u64 + 8192;
    TSS.ist[2] = ist3_top;

    GDT.entries[0] = 0;
    GDT.entries[1] = make_segment(0, true);
    GDT.entries[2] = make_segment(0, false);
    GDT.entries[3] = make_segment(3, false);
    GDT.entries[4] = make_segment(3, true);

    let tss_addr = core::ptr::addr_of!(TSS) as u64;
    let (tss_lo, tss_hi) = make_tss_descriptor(tss_addr, (core::mem::size_of::<Tss>() - 1) as u16);
    GDT.entries[5] = tss_lo;
    GDT.entries[6] = tss_hi;

    let gdtr = Gdtr {
        limit: (core::mem::size_of::<Gdt>() - 1) as u16,
        base: core::ptr::addr_of!(GDT) as u64,
    };

    asm!("lgdt [{}]", in(reg) &gdtr);

    // Far return to reload CS selector — skip far jump for now
    // and just reload data segments by hand.
    // The CS is already correct for our GDT layout.

    asm!(
        "mov ds, {0:x}",
        "mov es, {0:x}",
        "mov ss, {0:x}",
        "mov fs, {0:x}",
        "mov gs, {0:x}",
        in(reg) KERNEL_DS as u64,
    );

    asm!("ltr {0:x}", in(reg) TSS_SEL as u64);
}

// ═══════════════════════════════════════════════════════════════════════════
// IDT — Interrupt Descriptor Table (256 entries)
// ═══════════════════════════════════════════════════════════════════════════

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const fn empty() -> Self {
        Self { offset_low: 0, selector: 0, ist: 0, type_attr: 0, offset_mid: 0, offset_high: 0, reserved: 0 }
    }

    fn set_handler(&mut self, handler: u64, ist: u8) {
        self.offset_low = handler as u16;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.selector = 0x08;
        self.ist = ist;
        self.type_attr = 0x8E;
        self.reserved = 0;
    }
}

#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u64,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry::empty(); 256];

// ── Exception handlers ───────────────────────────────────────────────

extern "x86-interrupt" fn exc_no_err(_sf: u64) {
    serial_write("[stage1] EXCEPTION (no err) — halting\n");
    loop { unsafe { asm!("hlt"); } }
}

extern "x86-interrupt" fn exc_err(_sf: u64, error: u64) {
    serial_write("[stage1] EXCEPTION error=0x");
    serial_write_u64(error, 4);
    serial_write(" — halting\n");
    loop { unsafe { asm!("hlt"); } }
}

extern "x86-interrupt" fn exc_divide_error(_sf: u64) {
    serial_write("[stage1] #DE Divide Error — halting\n");
    loop { unsafe { asm!("hlt"); } }
}

extern "x86-interrupt" fn exc_invalid_opcode(_sf: u64) {
    serial_write("[stage1] #UD Invalid Opcode — halting\n");
    loop { unsafe { asm!("hlt"); } }
}

extern "x86-interrupt" fn exc_device_not_avail(_sf: u64) {
    serial_write("[stage1] #NM Device Not Available — halting\n");
    loop { unsafe { asm!("hlt"); } }
}

extern "x86-interrupt" fn exc_double_fault(_sf: u64, error: u64) {
    serial_write("[stage1] #DF Double Fault error=0x");
    serial_write_u64(error, 4);
    serial_write("\n");
    loop { unsafe { asm!("cli; hlt"); } }
}

extern "x86-interrupt" fn exc_gpf(_sf: u64, error: u64) {
    serial_write("[stage1] #GP General Protection error=0x");
    serial_write_u64(error, 4);
    serial_write("\n");
    loop { unsafe { asm!("hlt"); } }
}

extern "x86-interrupt" fn exc_page_fault(_sf: u64, error: u64) {
    let cr2: u64;
    unsafe { asm!("mov {}, cr2", out(reg) cr2); }
    serial_write("[stage1] #PF CR2=0x");
    serial_write_u64(cr2, 16);
    serial_write(" error=0x");
    serial_write_u64(error, 4);
    serial_write("\n");
    loop { unsafe { asm!("hlt"); } }
}

extern "x86-interrupt" fn exc_x87_fp(_sf: u64) {
    serial_write("[stage1] #MF x87 FP Exception — halting\n");
    loop { unsafe { asm!("hlt"); } }
}

extern "x86-interrupt" fn exc_machine_check(_sf: u64) {
    serial_write("[stage1] #MC Machine Check — halting\n");
    loop { unsafe { asm!("cli; hlt"); } }
}

extern "x86-interrupt" fn exc_simd_fp(_sf: u64) {
    serial_write("[stage1] #XM SIMD Exception — halting\n");
    loop { unsafe { asm!("hlt"); } }
}

extern "x86-interrupt" fn irq_stub(_sf: u64) {
    // Spurious or unused IRQ — ignore
}

unsafe fn init_idt() {
    macro_rules! fn_addr {
        ($f:expr) => { $f as *const () as u64 }
    }

    // Vectors WITHOUT error code: 0-7,9,15,16,18-20,22-31
    let no_err_vecs = [0,1,2,3,4,5,6,7,9,15,16,18,19,20,22,23,24,25,26,27,28,31];
    for &v in &no_err_vecs {
        let (handler, ist) = match v {
            0  => (fn_addr!(exc_divide_error), 1),
            6  => (fn_addr!(exc_invalid_opcode), 1),
            7  => (fn_addr!(exc_device_not_avail), 1),
            16 => (fn_addr!(exc_x87_fp), 1),
            18 => (fn_addr!(exc_machine_check), 3),
            19 => (fn_addr!(exc_simd_fp), 1),
            2  => (fn_addr!(exc_no_err), 1),  // NMI
            _  => (fn_addr!(exc_no_err), 0),
        };
        IDT[v].set_handler(handler, ist);
    }

    // Vectors WITH error code: 8,10,11,12,13,14,17,21,29,30
    let err_vecs = [8,10,11,12,13,14,17,21,29,30];
    for &v in &err_vecs {
        let (handler, ist) = match v {
            8  => (fn_addr!(exc_double_fault), 1),
            13 => (fn_addr!(exc_gpf), 1),
            14 => (fn_addr!(exc_page_fault), 1),
            _  => (fn_addr!(exc_err), 0),
        };
        IDT[v].set_handler(handler, ist);
    }

    // IRQs (32-255) — simple iretq stub
    for v in 32..256 {
        IDT[v].set_handler(fn_addr!(irq_stub), 0);
    }

    let idtr = Idtr {
        limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: core::ptr::addr_of!(IDT) as u64,
    };
    asm!("lidt [{}]", in(reg) &idtr);
}

// ═══════════════════════════════════════════════════════════════════════════
// SYSCALL MSRs — STAR, LSTAR, FMASK, EFER.SCE
// ═══════════════════════════════════════════════════════════════════════════

const IA32_EFER: u32 = 0xC0000080;
const IA32_STAR: u32 = 0xC0000081;
const IA32_LSTAR: u32 = 0xC0000082;
const IA32_FMASK: u32 = 0xC0000084;

unsafe fn wrmsr(msr: u32, val: u64) {
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    asm!("wrmsr", in("ecx") msr, in("eax") lo, in("edx") hi);
}

unsafe fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi);
    ((hi as u64) << 32) | (lo as u64)
}

#[no_mangle]
#[link_section = ".text.syscall_entry"]
pub extern "C" fn syscall_entry_stub() {
    // Minimal syscall stub: saved by kernel, just return for now
    unsafe {
        asm!("sysretq", options(noreturn));
    }
}

unsafe fn init_syscall(ctx: &mut BootContext) {
    let efer = rdmsr(IA32_EFER);
    wrmsr(IA32_EFER, efer | 1); // SCE

    let star = (KERNEL_DS as u64) << 48 | (KERNEL_CS as u64) << 32;
    wrmsr(IA32_STAR, star);

    let entry = syscall_entry_stub as *const () as u64;
    wrmsr(IA32_LSTAR, entry);
    wrmsr(IA32_FMASK, (1 << 9) | (1 << 10)); // mask IF + DF

    ctx.syscall_entry = entry;
}

// ═══════════════════════════════════════════════════════════════════════════
// CPU — feature detection, registers, FPU, TSC, cache
// ═══════════════════════════════════════════════════════════════════════════

fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (eax, ebx, ecx, edx);
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") leaf => eax,
            inout("ecx") sub => ecx,
            ebx_out = out(reg) ebx,
            out("edx") edx,
        );
    }
    (eax, ebx, ecx, edx)
}

fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;
    unsafe { asm!("rdtsc", out("eax") low, out("edx") high); }
    ((high as u64) << 32) | low as u64
}

unsafe fn read_cr0() -> u64 {
    let v: u64;
    asm!("mov {}, cr0", out(reg) v);
    v
}

unsafe fn read_cr4() -> u64 {
    let v: u64;
    asm!("mov {}, cr4", out(reg) v);
    v
}

fn detect_vendor() -> [u8; 12] {
    let (_, ebx, ecx, edx) = cpuid(0, 0);
    let mut vendor = [0u8; 12];
    vendor[0..4].copy_from_slice(&ebx.to_ne_bytes());
    vendor[4..8].copy_from_slice(&edx.to_ne_bytes());
    vendor[8..12].copy_from_slice(&ecx.to_ne_bytes());
    vendor
}

fn is_vendor(vendor: &[u8; 12], expected: &[u8; 12]) -> bool {
    vendor == expected
}

const VENDOR_INTEL: [u8; 12] = *b"GenuineIntel";
const VENDOR_AMD: [u8; 12] = *b"AuthenticAMD";

fn cpu_has_fsgsbase() -> bool {
    cpuid(7, 0).1 & (1 << 0) != 0
}

fn cpu_has_smep() -> bool {
    cpuid(7, 0).1 & (1 << 7) != 0
}

fn cpu_has_smap() -> bool {
    cpuid(7, 0).1 & (1 << 20) != 0
}

fn cpu_has_xsave() -> bool {
    cpuid(1, 0).2 & (1 << 26) != 0
}

fn cpu_has_osxsave() -> bool {
    cpuid(1, 0).2 & (1 << 27) != 0
}

fn cpu_has_avx() -> bool {
    cpuid(1, 0).2 & (1 << 28) != 0
}

fn cpu_has_avx2() -> bool {
    cpuid(7, 0).1 & (1 << 5) != 0
}

fn cpu_has_mtrr() -> bool {
    cpuid(1, 0).3 & (1 << 12) != 0
}

fn cpu_has_umip() -> bool {
    cpuid(7, 0).2 & (1 << 2) != 0
}

unsafe fn init_cr0_cr4() {
    let mut cr0 = read_cr0();
    cr0 |= 1 << 1;     // MP
    cr0 &= !(1 << 2);  // clear EM
    cr0 |= 1 << 5;     // NE
    cr0 &= !(1 << 16); // clear WP
    cr0 &= !(1 << 3);  // clear TS
    asm!("mov cr0, {}", in(reg) cr0);

    let mut cr4 = read_cr4();
    cr4 |= 1 << 9;     // OSFXSR
    cr4 |= 1 << 10;    // OSXMMEXCPT
    if cpu_has_avx() && cpu_has_osxsave() {
        cr4 |= 1 << 18; // OSXSAVE
    }
    if cpu_has_fsgsbase() {
        cr4 |= 1 << 16; // FSGSBASE
    }
    if cpu_has_smep() {
        cr4 |= 1 << 20; // SMEP
    }
    if cpu_has_umip() {
        cr4 |= 1 << 11; // UMIP
    }
    asm!("mov cr4, {}", in(reg) cr4);

    serial_write("[cpu] CR0/CR4 configured\n");
}

unsafe fn init_xcr0() {
    if !cpu_has_avx() || !cpu_has_osxsave() {
        serial_write("[cpu] XCR0: skipped (no AVX/OSXSAVE)\n");
        return;
    }
    if read_cr4() & (1 << 18) == 0 {
        serial_write("[cpu] XCR0: FAIL — CR4.OSXSAVE not set!\n");
        return;
    }
    let xcr0: u64 = (1 << 0) | (1 << 1) | (1 << 2); // x87 | SSE | AVX
    let eax = (xcr0 & 0xFFFFFFFF) as u32;
    let edx = (xcr0 >> 32) as u32;
    asm!("xsetbv", in("ecx") 0u32, in("eax") eax, in("edx") edx);
    serial_write("[cpu] XCR0 configured (x87 + SSE + AVX)\n");
}

static mut INITIAL_FPU_STATE: [u8; 1024] = [0; 1024];

unsafe fn init_fpu() {
    asm!("fninit");

    let mxcsr: u32 = 0x1F80;
    asm!("ldmxcsr [{addr}]", addr = in(reg) &mxcsr as *const u32);

    let ptr = core::ptr::addr_of_mut!(INITIAL_FPU_STATE) as *mut u8;
    let mask_lo: u32 = 0x7; // x87 | SSE | AVX
    asm!("xsave [{}]", in(reg) ptr, in("eax") mask_lo, in("edx") 0u32);

    serial_write("[cpu] FPU + MXCSR initialized\n");
}

fn calibrate_tsc() -> u64 {
    // Try CPUID leaf 0x15 (Core Crystal Clock)
    let (eax, _ebx, ecx, _edx) = cpuid(0x15, 0);
    let freq = if eax != 0 && ecx != 0 {
        ecx as u64
    } else {
        // Fallback: Ryzen 5 5600X base clock
        3_700_000_000
    };

    serial_write("[cpu] TSC calibrated: ");
    print_freq(freq);
    serial_write(" Hz\n");

    freq
}

fn print_freq(freq: u64) {
    let mut buf = [0u8; 20];
    let mut v = freq;
    let mut i = buf.len();
    if v == 0 {
        i -= 1; buf[i] = b'0';
    } else {
        while v > 0 {
            i -= 1;
            buf[i] = b'0' + (v % 10) as u8;
            v /= 10;
        }
    }
    if let Ok(s) = core::str::from_utf8(&buf[i..]) {
        serial_write(s);
    }
}

unsafe fn init_cache() {
    if cpu_has_mtrr() {
        // Set default memory type to Write-Back (WB)
        let def = rdmsr(0x2FF); // IA32_MTRR_DEF_TYPE
        let def = (def & !0xFF) | 6; // MTRR_TYPE_WB
        wrmsr(0x2FF, def | 0x800); // enable MTRRs

        serial_write("[cpu] MTRRs configured (default WB)\n");
    } else {
        serial_write("[cpu] MTRR: not supported\n");
    }
    serial_write("[cpu] PAT: default config OK\n");
}

fn print_brand_string() {
    let (a, b, c, d) = cpuid(0x80000002, 0);
    let (e, f, g, h) = cpuid(0x80000003, 0);
    let (i, j, k, l) = cpuid(0x80000004, 0);
    let mut buf = [0u8; 48];
    let mut idx = 0;
    for (a, b, c, d) in [(a, b, c, d), (e, f, g, h), (i, j, k, l)] {
        for &v in &[a, b, c, d] {
            if idx < 48 {
                buf[idx] = v as u8; idx += 1;
                if v > 0xFF { buf[idx] = (v >> 8) as u8; idx += 1; }
                if v > 0xFFFF { buf[idx] = (v >> 16) as u8; idx += 1; }
                if v > 0xFFFFFF { buf[idx] = (v >> 24) as u8; idx += 1; }
            }
        }
    }
    serial_write("[cpu] ");
    if let Ok(s) = core::str::from_utf8(&buf[..idx.min(48)]) {
        let trimmed = s.trim_end_matches('\0').trim_end();
        serial_write(trimmed);
    }
    serial_write("\n");
}

// ═══════════════════════════════════════════════════════════════════════════
// Entry Point
// ═══════════════════════════════════════════════════════════════════════════

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut BootContext) -> ! {
    let ctx = unsafe { &mut *ctx_ptr };

    serial_init();
    serial_write("\n[stage1] Arch init — GDT, IDT, SYSCALL, CPU\n");

    // ── 1. GDT ─────────────────────────────────────────────
    unsafe {
        init_gdt();
    }
    serial_write("[stage1] GDT loaded with TSS + IST stacks\n");

    // ── 2. Store GDT/IDT pointers ─────────────────────────
    ctx.gdt_ptr = core::ptr::addr_of!(GDT) as u64;
    ctx.tss_ptr = core::ptr::addr_of!(TSS) as u64;
    ctx.kernel_stack_top = core::ptr::addr_of!(KERNEL_STACK) as u64 + 16384;

    // ── 3. IDT ─────────────────────────────────────────────
    unsafe { init_idt(); }
    serial_write("[stage1] IDT loaded (256 entries)\n");
    ctx.idt_ptr = core::ptr::addr_of!(IDT) as u64;

    // ── 4. CPU detection ───────────────────────────────────
    let vendor = detect_vendor();
    serial_write("[cpu] Vendor: ");
    if let Ok(s) = core::str::from_utf8(&vendor) {
        serial_write(s.trim_end_matches('\0'));
    }
    serial_write("\n");

    print_brand_string();

    serial_write("[cpu] XSAVE=");
    serial_write(if cpu_has_xsave() { "Y" } else { "N" });
    serial_write(" SMEP=");
    serial_write(if cpu_has_smep() { "Y" } else { "N" });
    serial_write(" FSGSBASE=");
    serial_write(if cpu_has_fsgsbase() { "Y" } else { "N" });
    serial_write(" UMIP=");
    serial_write(if cpu_has_umip() { "Y" } else { "N" });
    serial_write("\n");

    // ── 5. CR0/CR4 ─────────────────────────────────────────
    unsafe { init_cr0_cr4(); }

    // ── 6. XCR0 ────────────────────────────────────────────
    unsafe { init_xcr0(); }

    // ── 7. FPU ─────────────────────────────────────────────
    unsafe { init_fpu(); }

    // ── 8. Cache (MTRR + PAT) ──────────────────────────────
    unsafe { init_cache(); }

    // ── 9. TSC calibration ─────────────────────────────────
    let tsc_freq = calibrate_tsc();
    ctx.tsc_freq = tsc_freq;

    // ── 10. SYSCALL ────────────────────────────────────────
    unsafe {
        init_syscall(ctx);
    }
    serial_write("[stage1] SYSCALL MSRs configured\n");

    serial_write("[stage1] Context updated, jumping to stage2\n");

    // ── Jump to Stage 2 ──────────────────────────────────
    let stage2_entry = ctx.stage_entry[1];
    if stage2_entry != 0 {
        unsafe {
            let stage2_fn: extern "C" fn(*mut BootContext) -> ! =
                core::mem::transmute(stage2_entry);
            stage2_fn(ctx_ptr);
        }
    }

    unsafe { asm!("hlt"); }
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { asm!("hlt"); } }
}
