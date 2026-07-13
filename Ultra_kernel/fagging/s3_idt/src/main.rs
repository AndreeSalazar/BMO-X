//! Faggin stage 3 — IDT (256 entries).
//!
//! Responsibilities (one only):
//!   - Define all 32 exception handlers (no-err and with-err) plus
//!     a single IRQ stub for vectors 32..255.
//!   - Load IDTR.
//!   - Publish idt_ptr in BootContext.
//!   - Jump to s4_cpuid.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;

extern "C" {
    fn s4_cpuid(ctx: *mut boot_context::BootContext) -> !;
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    off_lo: u16, sel: u16, ist: u8, attr: u8,
    off_mid: u16, off_hi: u32, _r: u32,
}

impl IdtEntry {
    const fn empty() -> Self { Self { off_lo: 0, sel: 0, ist: 0, attr: 0, off_mid: 0, off_hi: 0, _r: 0 } }
    fn set(&mut self, h: u64, ist: u8) {
        self.off_lo = h as u16;
        self.off_mid = (h >> 16) as u16;
        self.off_hi  = (h >> 32) as u32;
        self.sel = 0x08; self.ist = ist; self.attr = 0x8E; self._r = 0;
    }
}

#[repr(C, packed)] struct Idtr { limit: u16, base: u64 }
static mut IDT: [IdtEntry; 256] = [IdtEntry::empty(); 256];

// ── Exception handlers (all halt with a serial message) ──────────

macro_rules! halt_handler {
    ($name:ident, $msg:literal) => {
        extern "x86-interrupt" fn $name(_sf: u64) {
            serial_shared::puts(concat!("[s3 idt] ", $msg, " — halting\n"));
            loop { unsafe { asm!("hlt"); } }
        }
    };
}
halt_handler!(exc_no_err,      "EXCEPTION (no err)");
halt_handler!(exc_divide,      "#DE Divide Error");
halt_handler!(exc_invalid_op,   "#UD Invalid Opcode");
halt_handler!(exc_dev_not_av,  "#NM Device Not Available");
halt_handler!(exc_x87,         "#MF x87 FP");
halt_handler!(exc_simd,        "#XM SIMD");
halt_handler!(exc_mcheck,      "#MC Machine Check");
halt_handler!(exc_no_err2,     "EXCEPTION (no err, mirror)");
halt_handler!(irq_stub,        "IRQ stub");

extern "x86-interrupt" fn exc_with_err(_sf: u64, _e: u64) {
    serial_shared::puts("[s3 idt] EXCEPTION (with err) — halting\n");
    loop { unsafe { asm!("hlt"); } }
}

extern "x86-interrupt" fn exc_double_fault(_sf: u64, _e: u64) {
    serial_shared::puts("[s3 idt] #DF Double Fault — halting\n");
    loop { unsafe { asm!("cli; hlt"); } }
}

extern "x86-interrupt" fn exc_gpf(_sf: u64, _e: u64) {
    serial_shared::puts("[s3 idt] #GP — halting\n");
    loop { unsafe { asm!("hlt"); } }
}

extern "x86-interrupt" fn exc_page_fault(_sf: u64, _e: u64) {
    serial_shared::puts("[s3 idt] #PF — halting\n");
    loop { unsafe { asm!("hlt"); } }
}

unsafe fn init_idt() {
    macro_rules! fa { ($f:expr) => { $f as *const () as u64 } }

    let no_err = [0,1,2,3,4,5,6,7,9,15,16,18,19,20,22,23,24,25,26,27,28,31];
    for &v in &no_err {
        let (h, ist) = match v {
            0  => (fa!(exc_divide),     1u8),
            6  => (fa!(exc_invalid_op), 1),
            7  => (fa!(exc_dev_not_av), 1),
            16 => (fa!(exc_x87),        1),
            18 => (fa!(exc_mcheck),     3),
            19 => (fa!(exc_simd),       1),
            2  => (fa!(exc_no_err2),    1), // NMI
            _  => (fa!(exc_no_err),     0),
        };
        IDT[v].set(h, ist);
    }

    let err = [8,10,11,12,13,14,17,21,29,30];
    for &v in &err {
        let (h, ist) = match v {
            8  => (fa!(exc_double_fault), 1u8),
            13 => (fa!(exc_gpf),          1),
            14 => (fa!(exc_page_fault),   1),
            _  => (fa!(exc_with_err),     0),
        };
        IDT[v].set(h, ist);
    }
    for v in 32..256 { IDT[v].set(fa!(irq_stub), 0); }

    let idtr = Idtr {
        limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: core::ptr::addr_of!(IDT) as u64,
    };
    asm!("lidt [{}]", in(reg) &idtr);
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    unsafe { init_idt(); }
    serial_shared::puts("[s3 idt] 256 entries loaded\n");

    let ctx = unsafe { &mut *ctx_ptr };
    ctx.idt_ptr = unsafe { core::ptr::addr_of!(IDT) } as u64;

    unsafe {
        asm!(
            "jmp {next}",
            next = in(reg) s4_cpuid as *const () as u64,
            in("rdi") ctx_ptr,
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
