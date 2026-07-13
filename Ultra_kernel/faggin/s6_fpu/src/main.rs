//! Faggin stage 6 ??? FPU init.
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

const NEXT_ADDR: u64 = 0x160000;

#[repr(align(64))]
struct Align64([u8; 1024]);
static mut FPU_STATE: Align64 = Align64([0; 1024]);

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start(ctx_ptr: *mut boot_context::BootContext) -> ! {
    unsafe {
        asm!("fninit");
        let mxcsr: u32 = 0x1F80;
        asm!("ldmxcsr [{addr}]", addr = in(reg) &mxcsr as *const u32);
        let ptr = core::ptr::addr_of_mut!(FPU_STATE.0) as *mut u8;
        asm!("xsave [{}]", in(reg) ptr, in("eax") 0x7u32, in("edx") 0u32);
    }
    serial_shared::puts("[s6 fpu] FPU + MXCSR + XSAVE\n");

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
