//! App launcher / desktop shell -- stub.

#![no_std]

pub fn run() -> ! {
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}
