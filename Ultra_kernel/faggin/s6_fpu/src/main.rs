//! Faggin stage 6 — FPU init.
//!
//! Responsibilities (one only):
//!   - `fninit` (reset FPU).
//!   - `ldmxcsr` to 0x1F80.
//!   - `xsave` an initial FPU state.
//!   - Jump to s7_tsc.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;

extern "C" {
    fn s7_tsc(ctx: *mut boot_context::BootContext) -> !;
}

#[repr(C, align(64))]
static mut FPU_STATE: [u8; 1024] = [0; 1024];

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    unsafe {
        asm!("fninit");
        let mxcsr: u32 = 0x1F80;
        asm!("ldmxcsr [{addr}]", addr = in(reg) &mxcsr as *const u32);
        let ptr = core::ptr::addr_of_mut!(FPU_STATE) as *mut u8;
        asm!("xsave [{}]", in(reg) ptr, in("eax") 0x7u32, in("edx") 0u32);
    }
    serial_shared::puts("[s6 fpu] FPU + MXCSR + XSAVE\n");

    unsafe {
        asm!(
            "jmp {next}",
            next = in(reg) s7_tsc as *const () as u64,
            in("rdi") ctx_ptr,
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { asm!("hlt"); } } }
