//! Faggin stage 7 ??? TSC calibration.
//!
//! Responsibilities (one only):
//!   - Read CPUID leaf 0x15 (Core Crystal Clock).
//!   - Fall back to 3.7 GHz if not present.
//!   - Publish `tsc_freq` in BootContext.
//!   - Jump to s8_syscall.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;

const NEXT_ADDR: u64 = 0x170000;

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

fn calibrate() -> u64 {
    let (eax, _ebx, ecx, _edx) = cpuid(0x15, 0);
    if eax != 0 && ecx != 0 { ecx as u64 } else { 3_700_000_000 }
}

fn print_freq(freq: u64) {
    serial_shared::puts("[s7 tsc] ");
    serial_shared::dec(freq as usize);
    serial_shared::puts(" Hz\n");
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    let freq = calibrate();
    print_freq(freq);

    let ctx = unsafe { &mut *ctx_ptr };
    ctx.tsc_freq = freq;

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
