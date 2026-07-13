//! Terminal emulator — stub.
//!
//! Will become a PTY-backed shell that can launch ring3 processes.
//! In the base it just panics on init (no PTY yet).

#![no_std]

pub fn run() -> ! {
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}
