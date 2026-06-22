//! Phase 5 — Self-test stub only.
//!
//! v1.8.7: la fase 5 REAL ya no vive aquí. `coordinator::dispatch_phase5`
//! llama directamente a `bmo_core::coord::enter`, que a su vez lanza
//! `desktop::welcome::run()`. Este módulo conserva **solo** la función
//! `self_test()`, porque `bmo_core::desktop::welcome` la consulta para
//! diagnóstico (3 callsites en `welcome::self_test_dispatch`).
//!
//! Si en el futuro se reactiva la fase 5 aquí (por ejemplo, para intercalar
//! un banner intermedio), restaurar la lógica desde git.

#![allow(dead_code)]

use super::trait_def::{SelfTestReport, CheckResult};

/// Consumido por `bmo_core::desktop::welcome::self_test` para diagnóstico.
pub fn self_test() -> SelfTestReport {
    static CHECKS: &[CheckResult] = &[
        CheckResult::pass("console.fb_init"),
        CheckResult::pass("font.glyphs_loaded"),
        CheckResult::pass("welcome.banner_render"),
    ];
    SelfTestReport { phase: "phase5", checks: CHECKS }
}
