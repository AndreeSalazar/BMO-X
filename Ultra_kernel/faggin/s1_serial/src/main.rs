//! Faggin stage 1 — COM1 serial init.
//!
//! Responsibilities (one only):
//!   - Initialize COM1 at 115200 8N1.
//!   - Print the chain banner.
//!   - Jump to s2_gdt.
//!
//! Writes nothing to BootContext. The serial port is the first thing
//! every later stage uses to log progress.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

use core::panic::PanicInfo;
use core::arch::asm;

extern "C" {
    fn s2_gdt(ctx: *mut boot_context::BootContext) -> !;
}

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    serial_shared::init();
    serial_shared::puts("\n[s1 serial] BMO chain begin\n");

    unsafe {
        asm!(
            "jmp {next}",
            next = in(reg) s2_gdt as *const () as u64,
            in("rdi") ctx_ptr,
            options(noreturn)
        );
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { asm!("hlt"); } }
}
