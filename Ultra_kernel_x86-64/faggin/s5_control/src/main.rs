//! Faggin stage 5 ??? Control registers (CR0, CR4, XCR0).
//!
//! Responsibilities (one only):
//!   - Set CR0 bits: MP, NE; clear EM, WP, TS.
//!   - Set CR4 bits: OSFXSR, OSXMMEXCPT, OSXSAVE (if AVX),
//!     FSGSBASE, SMEP, UMIP.
//!   - Set XCR0: x87 + SSE + AVX (if supported).
//!   - Jump to s6_fpu.
//!
//! Writes nothing to BootContext.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;

const NEXT_ADDR: u64 = 0x150000;

#[inline]
fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (eax, ebx, ecx, edx);
    unsafe {
        asm!(
            "push rbx", "cpuid", "mov {ebx_out:e}, ebx", "pop rbx",
            inout("eax") leaf => eax, inout("ecx") sub => ecx,
            ebx_out = out(reg) ebx, out("edx") edx,
        );
    }
    (eax, ebx, ecx, edx)
}

unsafe fn init_cr0_cr4() {
    let cr0: u64;
    asm!("mov {}, cr0", out(reg) cr0);
    let mut cr0 = cr0;
    cr0 |= 1 << 1;     // MP
    cr0 &= !(1 << 2);  // clear EM
    cr0 |= 1 << 5;     // NE
    cr0 &= !(1 << 16); // clear WP
    cr0 &= !(1 << 3);  // clear TS
    asm!("mov cr0, {}", in(reg) cr0);

    let (max_basic, _, _, _) = cpuid(0, 0);
    let (_, _, ecx1, _) = cpuid(1, 0);
    let (ebx7, ecx7) = if max_basic >= 7 {
        let (_, ebx, ecx, _) = cpuid(7, 0);
        (ebx, ecx)
    } else {
        (0, 0)
    };
    let avx  = ecx1 & (1 << 28) != 0;
    let xsav = ecx1 & (1 << 26) != 0;
    let smep    = ebx7 & (1 << 7) != 0;
    let fsgs    = ebx7 & (1 << 0) != 0;
    let umip    = ecx7 & (1 << 2) != 0;

    let cr4: u64;
    asm!("mov {}, cr4", out(reg) cr4);
    let mut cr4 = cr4;
    cr4 |= 1 << 7;     // PGE ??? enable global pages (required by s9_paging PTE_GLOBAL)
    cr4 |= 1 << 9;     // OSFXSR
    cr4 |= 1 << 10;    // OSXMMEXCPT
    if xsav { cr4 |= 1 << 18; }           // OSXSAVE
    if fsgs { cr4 |= 1 << 16; }            // FSGSBASE
    if smep { cr4 |= 1 << 20; }            // SMEP
    if umip { cr4 |= 1 << 11; }           // UMIP
    asm!("mov cr4, {}", in(reg) cr4);

    // XCR0: x87 + SSE + AVX (if supported)
    if avx && xsav {
        let xcr0: u64 = (1 << 0) | (1 << 1) | (1 << 2);
        let eax = (xcr0 & 0xFFFFFFFF) as u32;
        let edx = (xcr0 >> 32) as u32;
        asm!("xsetbv", in("ecx") 0u32, in("eax") eax, in("edx") edx);
    }
    serial_shared::puts("[s5 control] CR0/CR4/XCR0 set\n");
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    unsafe { init_cr0_cr4(); }
    unsafe {
        asm!(
            "jmp {next}",
            next = in(reg) NEXT_ADDR,
            in("rdi") ctx_ptr,
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
