//! crt0 — C runtime startup for Ring 3 BEF apps.
//!
//! This is the first code executed when a BEF binary is loaded into
//! a Ring 3 process. Sets up the stack, calls main(), and exits.

use crate::string::memset;

extern "C" {
    static __bss_start: u8;
    static __bss_end: u8;
    fn main(argc: i32, argv: *const *const u8) -> i32;
}

/// Entry point. Called by the kernel after loading the BEF into Ring 3.
///
/// The kernel provides:
/// - Identity-mapped code + data pages (USER flag)
/// - A 64 KB stack
/// - RSP pointing to the top of the stack
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    // Zero-init BSS section
    let bss_start = &__bss_start as *const u8 as *mut u8;
    let bss_end = &__bss_end as *const u8 as usize;
    let bss_len = bss_end - bss_start as usize;
    if bss_len > 0 && bss_len < 64 * 1024 * 1024 {
        memset(bss_start, 0, bss_len);
    }

    // Call user's main function
    let ret = main(0, core::ptr::null());

    // Exit the process via syscall
    crate::syscall::proc_exit(ret as u32);

    // Safety: proc_exit should never return, but if it does:
    loop { unsafe { core::arch::asm!("pause"); } }
}
