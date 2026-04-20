//! FastOS UEFI Bootloader - Minimal Test
//!
//! Test that UEFI boot works. Just prints a message and halts.
//! Will expand to full bootloader once this compiles and boots.

#![no_std]
#![no_main]

extern crate alloc;

use uefi::prelude::*;
use log::info;

#[global_allocator]
static ALLOC: uefi::allocator::Allocator = uefi::allocator::Allocator;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}

#[entry]
fn main(_image: Handle, mut st: SystemTable<Boot>) -> Status {
    st.stdout().clear().unwrap();
    
    info!("[FastOS] UEFI Bootloader v1.0");
    info!("[FastOS] Board: MSI A320M-A PRO MAX (MS-7C52)");
    info!("[FastOS] CPU: Ryzen 5 5600X (Zen 3)");
    info!("[FastOS] Mode: 64-bit Long Mode (native UEFI)");
    info!("[FastOS] Test OK - Halting");
    
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}
