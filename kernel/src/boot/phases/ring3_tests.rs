//! Ring 3 transition tests.
//!
//! These tests verify the structural correctness of the Ring 0/3 machinery
//! without actually performing a real Ring 3 jump. They cover:
//!   - iretq frame layout
//!   - GDT selectors and STAR MSR
//!   - SYSCALL calling convention
//!   - Paging flags (NX bit, USER/SUPERVISOR)
//!   - IST1 stack size
//!   - TSS / SYSCALL consistency
//!   - User memory layout
//!   - swapgs / clac / stac opcodes
//!   - BMOasm codegen of syscall/mov/ret
//!
//! They run at two points:
//!   - `run_all_tests()`     — before Phase 1 (no heap)
//!   - `run_codegen_tests()` — after  Phase 1 (heap live)
//!
//! Both must pass before any real user-mode process is created.

use crate::{arch, boot::log};

pub fn run_all_tests() {
    log::info("ring3-test", "Running Ring 3 transition tests");
    crate::drivers::serial::serial_write("[probe] about to call run_all_tests\n");
    match arch::ring3_test::run_all_tests() {
        Ok(n) => {
            crate::drivers::serial::serial_write("[probe] run_all_tests returned OK\n");
            log::info_u64("ring3-test", "tests passed", n as u64);
        }
        Err(_) => {
            crate::drivers::serial::serial_write("[probe] run_all_tests returned Err\n");
            log::fault("ring3-test", "Ring 3 transition tests failed");
        }
    }
}

pub fn run_codegen_tests() {
    log::info("ring3-codegen", "Running BMOasm codegen tests (heap live)");
    let n = arch::ring3_test::run_codegen_tests();
    log::info_u64("ring3-codegen", "codegen tests passed", n as u64);
}
