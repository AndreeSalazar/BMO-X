//! ÑEXO std::sys — Llamadas al sistema BMO.

#![allow(dead_code)]

pub fn syscall(nr: u64, a1: u64, a2: u64, a3: u64) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a1,
            in("rsi") a2,
            in("rdx") a3,
            options(nomem, nostack)
        );
    }
    ret
}

pub fn debug_print(s: &str) { syscall(0xF0, s.as_ptr() as u64, s.len() as u64, 0); }
pub fn clock_get_time() -> u64 { syscall(0x50, 0, 0, 0) }
pub fn process_exit(code: i32) -> ! { syscall(0x00, code as u64, 0, 0); loop { unsafe { core::arch::asm!("hlt"); } } }
