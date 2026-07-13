//! Faggin stage 2 — GDT + TSS + kernel/IST stacks.
//!
//! Responsibilities (one only):
//!   - Define the GDT (null, kernel CS/DS, user DS/CS, TSS).
//!   - Define the TSS with kernel stack + IST1 (for #PF/#GP/#DF) and
//!     IST3 (for #MC).
//!   - Load GDTR and TR, reload DS/ES/SS/FS/GS to kernel selectors.
//!   - Publish gdt_ptr, tss_ptr, kernel_stack_top in BootContext.
//!   - Jump to s3_idt.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;

extern "C" {
    fn s3_idt(ctx: *mut boot_context::BootContext) -> !;
}

const KERNEL_CS: u16 = 0x08;
const KERNEL_DS: u16 = 0x10;
const USER_DS: u16  = 0x18 | 3;
const USER_CS: u16  = 0x20 | 3;
const TSS_SEL: u16  = 0x28;

const IST1_SIZE: usize = 8192;
const IST3_SIZE: usize = 8192;
const KSTACK_SIZE: usize = 16384;

#[repr(C, packed)]
struct Tss {
    _r0: u32,
    rsp: [u64; 3],
    _r1: u64,
    ist: [u64; 7],
    _r2: u64,
    _r3: u16,
    iomap_base: u16,
}

#[repr(C, align(16))]
struct Gdt { entries: [u64; 7] }

#[repr(C, packed)]
struct Gdtr { limit: u16, base: u64 }

#[repr(align(16))] struct IstStack([u8; IST1_SIZE]);   // 8 KB
#[repr(align(16))] struct McStack([u8; IST3_SIZE]);     // 8 KB
#[repr(align(16))] struct KernelStack([u8; KSTACK_SIZE]); // 16 KB

static mut TSS: Tss = Tss { _r0: 0, rsp: [0; 3], _r1: 0, ist: [0; 7], _r2: 0, _r3: 0, iomap_base: 0 };
static mut GDT: Gdt = Gdt { entries: [0; 7] };
static mut IST1: IstStack = IstStack([0; IST1_SIZE]);
static mut IST3: McStack  = McStack([0; IST3_SIZE]);
static mut KSTK: KernelStack = KernelStack([0; KSTACK_SIZE]);

fn make_segment(dpl: u8, code: bool) -> u64 {
    let mut d: u64 = 0xFFFF | (0x0F << 48);
    let mut a: u8 = 0x92 | (dpl << 5);
    if code { a |= 0x08; }
    d |= (a as u64) << 40;
    let mut f: u8 = if code { 0x0A } else { 0x0C };
    d |= (f as u64) << 52;
    d
}

fn make_tss_descriptor(addr: u64, size: u16) -> (u64, u64) {
    let mut lo: u64 = (size as u64) & 0xFFFF;
    lo |= (((size as u64) >> 16) & 0x0F) << 48;
    lo |= ((addr & 0xFFFF) as u64) << 16;
    lo |= (((addr >> 16) & 0xFF) as u64) << 32;
    lo |= (((addr >> 24) & 0xFF) as u64) << 56;
    lo |= 0x89u64 << 40;
    let hi: u64 = (addr >> 32) & 0xFFFFFFFF;
    (lo, hi)
}

unsafe fn init_gdt() {
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
    GDT.entries[5] = lo;
    GDT.entries[6] = hi;

    let gdtr = Gdtr {
        limit: (core::mem::size_of::<Gdt>() - 1) as u16,
        base: core::ptr::addr_of!(GDT) as u64,
    };
    asm!("lgdt [{}]", in(reg) &gdtr);
    asm!(
        "mov ds, {0:x}", "mov es, {0:x}", "mov ss, {0:x}",
        "mov fs, {0:x}", "mov gs, {0:x}",
        in(reg) KERNEL_DS as u64,
    );
    asm!("ltr {0:x}", in(reg) TSS_SEL as u64);
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    unsafe { init_gdt(); }
    serial_shared::puts("[s2 gdt] GDT + TSS + IST stacks loaded\n");

    let ctx = unsafe { &mut *ctx_ptr };
    ctx.gdt_ptr = unsafe { core::ptr::addr_of!(GDT) } as u64;
    ctx.tss_ptr = unsafe { core::ptr::addr_of!(TSS) } as u64;
    ctx.kernel_stack_top = unsafe { core::ptr::addr_of!(KSTK) } as u64 + KSTACK_SIZE as u64;

    unsafe {
        asm!(
            "jmp {next}",
            next = in(reg) s3_idt as *const () as u64,
            in("rdi") ctx_ptr,
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
