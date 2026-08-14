//! **GDT, TSS AND IDT** -- the tables the CPU reads to know what to do.
//!
//! === Why the tables and their init are one file ===
//!
//! They were 600 lines apart, and a descriptor table is meaningless without the
//! code that loads it: the layout and the `lgdt`/`lidt` have to agree byte for
//! byte, and keeping them where one cannot be changed without seeing the other
//! is the whole point.
//!
//! [!] Interrupts are masked before any of this runs. The firmware hands over
//! with them ENABLED, and an IRQ arriving mid-surgery dispatches through a
//! half-built table -- which is the triple fault that only ever happened on real
//! hardware, never in an emulator.

#[allow(unused_imports)]
use crate::*;

// ===================================================================
//  GDT + TSS (universal x86-64)
// ===================================================================

pub const KERNEL_CS: u16 = 0x08;
pub const KERNEL_DS: u16 = 0x10;
pub const USER_DS: u16  = 0x18 | 3;
pub const USER_CS: u16  = 0x20 | 3;
pub const TSS_SEL: u16  = 0x28;

pub const IST1_SIZE: usize = 8192;
pub const IST3_SIZE: usize = 8192;
pub const KSTACK_SIZE: usize = 16384;

#[repr(C, packed)] pub struct Tss { _r0: u32, rsp: [u64; 3], _r1: u64, ist: [u64; 7], _r2: u64, _r3: u16, iomap_base: u16 }
#[repr(C, align(16))] pub struct Gdt { entries: [u64; 7] }
#[repr(C, packed)] pub struct Gdtr { limit: u16, base: u64 }
#[repr(align(16))] pub struct IstStack([u8; IST1_SIZE]);
#[repr(align(16))] pub struct McStack([u8; IST3_SIZE]);
#[repr(align(16))] pub struct KernelStack([u8; KSTACK_SIZE]);

pub static mut TSS: Tss = Tss { _r0: 0, rsp: [0; 3], _r1: 0, ist: [0; 7], _r2: 0, _r3: 0, iomap_base: 0 };
pub static mut GDT: Gdt = Gdt { entries: [0; 7] };
pub static mut IST1: IstStack = IstStack([0; IST1_SIZE]);
pub static mut IST3: McStack  = McStack([0; IST3_SIZE]);
pub static mut KSTK: KernelStack = KernelStack([0; KSTACK_SIZE]);

// ===================================================================
//  IDT
// ===================================================================

#[repr(C, packed)] #[derive(Clone, Copy)]
pub struct IdtEntry { off_lo: u16, sel: u16, ist: u8, attr: u8, off_mid: u16, off_hi: u32, _r: u32 }
impl IdtEntry {
    const fn empty() -> Self { Self { off_lo: 0, sel: 0, ist: 0, attr: 0, off_mid: 0, off_hi: 0, _r: 0 } }
    fn set(&mut self, h: u64, ist: u8) {
        self.off_lo = h as u16; self.off_mid = (h >> 16) as u16; self.off_hi = (h >> 32) as u32;
        self.sel = 0x08; self.ist = ist; self.attr = 0x8E; self._r = 0;
    }
}
#[repr(C, packed)] pub struct Idtr { limit: u16, base: u64 }
pub static mut IDT: [IdtEntry; 256] = [IdtEntry::empty(); 256];

#[macro_export]
macro_rules! halt_handler { ($name:ident, $msg:literal) => { extern "x86-interrupt" fn $name(_sf: u64) { unsafe { put_str(concat!("[s1_cpu] ", $msg, " -- halting\n")); } loop { unsafe { asm!("hlt"); } } } }; }
halt_handler!(exc_no_err,      "EXCEPTION (no err)");
halt_handler!(exc_divide,      "#DE Divide Error");
halt_handler!(exc_invalid_op,   "#UD Invalid Opcode");
halt_handler!(exc_dev_not_av,  "#NM Device Not Available");
halt_handler!(exc_x87,         "#MF x87 FP");
halt_handler!(exc_simd,        "#XM SIMD");
halt_handler!(exc_mcheck,      "#MC Machine Check");
halt_handler!(exc_no_err2,     "EXCEPTION (no err, mirror)");
halt_handler!(irq_stub,        "IRQ stub");

pub extern "x86-interrupt" fn exc_with_err(_sf: u64, _e: u64) { unsafe { put_str("[s1_cpu] EXCEPTION (with err) -- halting\n"); } loop { unsafe { asm!("hlt"); } } }
pub extern "x86-interrupt" fn exc_double_fault(_sf: u64, _e: u64) { unsafe { put_str("[s1_cpu] #DF Double Fault -- halting\n"); } loop { unsafe { asm!("cli; hlt"); } } }
pub extern "x86-interrupt" fn exc_gpf(_sf: u64, _e: u64) { unsafe { put_str("[s1_cpu] #GP General Protection -- halting\n"); } loop { unsafe { asm!("hlt"); } } }
pub extern "x86-interrupt" fn exc_page_fault(_sf: u64, _e: u64) { unsafe { put_str("[s1_cpu] #PF Page Fault -- halting\n"); } loop { unsafe { asm!("hlt"); } } }


// ===================================================================
//  GDT / IDT init
// ===================================================================

pub fn make_segment(dpl: u8, code: bool) -> u64 {
    let mut d: u64 = 0xFFFF | (0x0F << 48);
    let mut a: u8 = 0x92 | (dpl << 5);
    if code { a |= 0x08; }
    d |= (a as u64) << 40;
    let f: u8 = if code { 0x0A } else { 0x0C };
    d |= (f as u64) << 52;
    d
}

pub fn make_tss_descriptor(addr: u64, size: u16) -> (u64, u64) {
    let mut lo: u64 = (size as u64) & 0xFFFF;
    lo |= (((size as u64) >> 16) & 0x0F) << 48;
    lo |= ((addr & 0xFFFF) as u64) << 16;
    lo |= (((addr >> 16) & 0xFF) as u64) << 32;
    lo |= (((addr >> 24) & 0xFF) as u64) << 56;
    lo |= 0x89u64 << 40;
    let hi: u64 = (addr >> 32) & 0xFFFFFFFF;
    (lo, hi)
}

pub unsafe fn init_gdt() {
    let ktop = core::ptr::addr_of!(KSTK) as u64 + KSTACK_SIZE as u64;
    TSS.rsp[0] = ktop;
    TSS.ist[0] = core::ptr::addr_of!(IST1) as u64 + IST1_SIZE as u64;
    TSS.ist[2] = core::ptr::addr_of!(IST3) as u64 + IST3_SIZE as u64;
    TSS.iomap_base = core::mem::size_of::<Tss>() as u16;
    GDT.entries[0] = 0;
    GDT.entries[1] = make_segment(0, true);
    GDT.entries[2] = make_segment(0, false);
    GDT.entries[3] = make_segment(3, false);
    GDT.entries[4] = make_segment(3, true);
    let tss_addr = core::ptr::addr_of!(TSS) as u64;
    let (lo, hi) = make_tss_descriptor(tss_addr, (core::mem::size_of::<Tss>() - 1) as u16);
    GDT.entries[5] = lo; GDT.entries[6] = hi;
    let gdtr = Gdtr { limit: (core::mem::size_of::<Gdt>() - 1) as u16, base: core::ptr::addr_of!(GDT) as u64 };
    // lgdt alone does NOT reload CS: the CPU keeps executing on the stale
    // UEFI code descriptor cached in the CS shadow register (cs=0x38 on this
    // firmware). That works silently -- until anything RE-validates that
    // selector against OUR table (the first trap-return iretq), which finds
    // entry 7 empty and #GPs with err=0x38. It also poisons every
    // `cmp cs, 0x08` kernel-vs-user check in the trap stubs (spurious
    // swapgs). The canonical fix: far-return through the new GDT so CS
    // becomes KERNEL_CS right here. retfq pops RIP, then CS.
    asm!(
        "lgdt [{gdtr}]",
        "push {kcs}",
        "lea {tmp}, [rip + 55f]",
        "push {tmp}",
        "retfq",
        "55:",
        gdtr = in(reg) &gdtr,
        kcs = in(reg) KERNEL_CS as u64,
        tmp = out(reg) _,
    );
    asm!("mov ds, {0:x}", "mov es, {0:x}", "mov ss, {0:x}", "mov fs, {0:x}", "mov gs, {0:x}", in(reg) KERNEL_DS as u64);
    asm!("ltr {0:x}", in(reg) TSS_SEL as u64);
}

pub unsafe fn init_idt() {
    macro_rules! fa { ($f:expr) => { $f as *const () as u64 } }
    let no_err = [0,1,2,3,4,5,6,7,9,15,16,18,19,20,22,23,24,25,26,27,28,31];
    for &v in &no_err {
        let (h, ist) = match v {
            0 => (fa!(exc_divide), 1u8), 6 => (fa!(exc_invalid_op), 1),
            7 => (fa!(exc_dev_not_av), 1), 16 => (fa!(exc_x87), 1),
            18 => (fa!(exc_mcheck), 3), 19 => (fa!(exc_simd), 1),
            2 => (fa!(exc_no_err2), 1), _ => (fa!(exc_no_err), 0),
        };
        IDT[v].set(h, ist);
    }
    let err = [8,10,11,12,13,14,17,21,29,30];
    for &v in &err {
        let (h, ist) = match v {
            8 => (fa!(exc_double_fault), 1u8), 13 => (fa!(exc_gpf), 1),
            14 => (fa!(exc_page_fault), 1), _ => (fa!(exc_with_err), 0),
        };
        IDT[v].set(h, ist);
    }
    for v in 32..256 { IDT[v].set(fa!(irq_stub), 0); }
    let idtr = Idtr { limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16, base: core::ptr::addr_of!(IDT) as u64 };
    asm!("lidt [{}]", in(reg) &idtr);
}
