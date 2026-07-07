//! Tests integrados del `bmo_core::desktop3`.
//!
//! Valida que la única puerta Ring 0 → BMO Core funciona correctamente.

#![allow(dead_code)]

use crate::desktop3;
use crate::bmo_abi::syscalls;

pub struct TestResult {
    pub name: &'static str,
    pub passed: bool,
    pub message: alloc::string::String,
}

pub fn run_all() -> alloc::vec::Vec<TestResult> {
    let mut r = alloc::vec::Vec::new();
    r.push(test_out_of_range_low());
    r.push(test_out_of_range_high());
    r.push(test_valid_syscall_time());
    r.push(test_valid_syscall_proc_get_pid());
    r.push(test_valid_syscall_audio_beep());
    r.push(test_valid_syscall_debug_print());
    r.push(test_stats_increment());
    r
}

fn test_out_of_range_low() -> TestResult {
    let total_before = desktop3::total();
    let r = desktop3::enter(0x050, 0, 0, 0, 0, 0, 0);
    if r == 0xFFFF_FFFF_FFFF_FFFF || r > 0x1000 {
        pass("out_of_range_low", &alloc::format!("nr=0x050 rejected, total={}->{}", total_before, desktop3::total()))
    } else {
        fail("out_of_range_low", &alloc::format!("nr=0x050 returned {}", r))
    }
}

fn test_out_of_range_high() -> TestResult {
    let r = desktop3::enter(0x300, 0, 0, 0, 0, 0, 0);
    if r == 0xFFFF_FFFF_FFFF_FFFF || r > 0x1000 {
        pass("out_of_range_high", "nr=0x300 rejected")
    } else {
        fail("out_of_range_high", &alloc::format!("nr=0x300 returned {}", r))
    }
}

fn test_valid_syscall_time() -> TestResult {
    let r = desktop3::enter(syscalls::NR_TIME_NOW_NS as u16, 0, 0, 0, 0, 0, 0);
    // TIME_NOW_NS retorna un u64, no debe ser INVALID_HANDLE ni nada
    if r != 0xFFFF_FFFF_FFFF_FFFF && r < 0x1000_0000_0000_0000 {
        pass("valid_syscall_time", &alloc::format!("time_ns={}", r))
    } else {
        fail("valid_syscall_time", &alloc::format!("returned {}", r))
    }
}

fn test_valid_syscall_proc_get_pid() -> TestResult {
    let r = desktop3::enter(syscalls::NR_PROC_GET_PID as u16, 0, 0, 0, 0, 0, 0);
    // El PID actual es 0 o algún número pequeño.
    if r < 0x100 {
        pass("valid_syscall_proc_get_pid", &alloc::format!("pid={}", r))
    } else {
        fail("valid_syscall_proc_get_pid", &alloc::format!("returned {}", r))
    }
}

fn test_valid_syscall_audio_beep() -> TestResult {
    let r = desktop3::enter(syscalls::NR_AUDIO_BEEP as u16, 1000, 50, 0, 0, 0, 0);
    // OK = 0.
    if r == 0 {
        pass("valid_syscall_audio_beep", "beep 1000Hz/50ms OK")
    } else {
        fail("valid_syscall_audio_beep", &alloc::format!("returned {}", r))
    }
}

fn test_valid_syscall_debug_print() -> TestResult {
    // "AB" en ptr NULL → retorna INVALID.
    let r = desktop3::enter(syscalls::NR_DEBUG_PRINT as u16, 0, 2, 0, 0, 0, 0);
    // INVALID = 0x1001 (InvalidArgument).
    if r != 0 {
        pass("valid_syscall_debug_print", &alloc::format!("null ptr rejected with code=0x{:x}", r))
    } else {
        fail("valid_syscall_debug_print", "null ptr accepted?")
    }
}

fn test_stats_increment() -> TestResult {
    let t0 = desktop3::total();
    let a0 = desktop3::allowed();
    let d0 = desktop3::denied();
    let u0 = desktop3::unknown();

    desktop3::enter(syscalls::NR_TIME_NOW_NS as u16, 0, 0, 0, 0, 0, 0);
    desktop3::enter(syscalls::NR_AUDIO_BEEP as u16, 500, 10, 0, 0, 0, 0);
    desktop3::enter(0x050, 0, 0, 0, 0, 0, 0);
    desktop3::enter(0x300, 0, 0, 0, 0, 0, 0);

    let t1 = desktop3::total();
    let a1 = desktop3::allowed();
    let d1 = desktop3::denied();
    let u1 = desktop3::unknown();

    let dt = t1 - t0;
    let da = a1 - a0;
    let du = u1 - u0;

    if dt >= 4 && du >= 2 && da >= 2 {
        pass("stats_increment", &alloc::format!("+{} total, +{} allowed, +{} unknown", dt, da, du))
    } else {
        fail("stats_increment",
             &alloc::format!("t+{} a+{} d+{} u+{}", dt, da, d1.saturating_sub(d0), du))
    }
}

// ── Helpers ──────────────────────────────────────────────────────

fn pass(name: &'static str, msg: &str) -> TestResult {
    TestResult { name, passed: true, message: alloc::string::String::from(msg) }
}
fn fail(name: &'static str, msg: &str) -> TestResult {
    TestResult { name, passed: false, message: alloc::string::String::from(msg) }
}

