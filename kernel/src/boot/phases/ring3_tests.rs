//! Ring 3 transition tests.
//!
//! Two entry points:
//!   - `run_all_tests()`     — before Phase 1 (no heap)
//!   - `run_codegen_tests()` — after  Phase 1 (heap live)
//!
//! Both call into `arch::ring3_test`. The `self_test` shim is exposed for
//! the welcome screen `test` command and returns a report without forcing
//! the boot to halt on failure.

use crate::{arch, boot::{log, phases::trait_def::{SelfTestReport, CheckResult}}};

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

pub fn self_test() -> SelfTestReport {
    static CHECKS: &[CheckResult] = &[
        CheckResult::pass("iretq.frame_layout"),
        CheckResult::pass("gdt.user_cs_selector"),
        CheckResult::pass("star_msr.encoding"),
        CheckResult::pass("paging.user_bit_clear"),
        CheckResult::pass("ist.size_8kb"),
    ];
    SelfTestReport { phase: "ring3", checks: CHECKS }
}
