//! Faggin stage 8 ??? SYSCALL MSRs (STAR, LSTAR, FMASK, EFER.SCE).
//!
//! Responsibilities (one only):
//!   - Enable EFER.SCE.
//!   - Program STAR with kernel CS/DS in the high half.
//!   - Program LSTAR with a syscall entry stub.
//!   - Program FMASK to mask IF + DF.
//!   - Publish `syscall_entry` in BootContext.
//!   - Jump to s9_paging.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;

const NEXT_ADDR: u64 = 0x180000;

const IA32_EFER:  u32 = 0xC0000080;
const IA32_STAR:  u32 = 0xC0000081;
const IA32_LSTAR: u32 = 0xC0000082;
const IA32_FMASK: u32 = 0xC0000084;

const KERNEL_CS: u64 = 0x08;
const KERNEL_DS: u64 = 0x10;

#[no_mangle]
#[link_section = ".text.syscall_entry"]
pub extern "C" fn syscall_entry_stub() {
    // Minimal stub: returns to userspace via `sysretq`.
    unsafe { asm!("sysretq", options(noreturn)); }
}

unsafe fn wrmsr(msr: u32, val: u64) {
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    asm!("wrmsr", in("ecx") msr, in("eax") lo, in("edx") hi);
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    // EFER read
    let efer_lo: u32; let efer_hi: u32;
    unsafe { asm!("rdmsr", in("ecx") IA32_EFER, out("eax") efer_lo, out("edx") efer_hi); }
    let efer = ((efer_hi as u64) << 32) | (efer_lo as u64);
    unsafe { wrmsr(IA32_EFER, efer | 1); }

    let star = (KERNEL_DS << 48) | (KERNEL_CS << 32);
    unsafe { wrmsr(IA32_STAR, star); }
    let entry = syscall_entry_stub as *const () as u64;
    unsafe { wrmsr(IA32_LSTAR, entry); }
    unsafe { wrmsr(IA32_FMASK, (1 << 9) | (1 << 10)); }

    let ctx = unsafe { &mut *ctx_ptr };
    ctx.syscall_entry = entry;

    serial_shared::puts("[s8 syscall] STAR/LSTAR/FMASK programmed\n");

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
