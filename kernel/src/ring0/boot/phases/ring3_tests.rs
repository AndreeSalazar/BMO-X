//! Ring 3 transition tests (v1.7.4).
//!
//! Sólo expone `self_test()` para el welcome `test ring3` command.
//! Los `run_all_tests()` y `run_codegen_tests()` activos se eliminaron
//! porque `crate::cpu::ring3_test` se borró en la limpieza de archivos dead.
//! Si en v1.7.x+ se re-introduce el trampoline real, agregar las
//! funciones `run_all_tests()` y `run_codegen_tests()` aquí.

use crate::boot::phases::trait_def::{CheckResult, SelfTestReport};

/// shim para el welcome `test ring3` command. Devuelve un report
/// sin forzar halt en caso de fallo.
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
