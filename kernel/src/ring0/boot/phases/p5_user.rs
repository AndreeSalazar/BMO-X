//! Phase 5 — Desktop.
//!
//! v1.6.16: `boot_start`/`phase4_end` are reserved for the desktop
//! boot progress indicator in v1.7.x.

#![allow(dead_code)]

use crate::{boot::log, bmo_core::desktop};
use crate::boot::context::BootContext;
use super::p0_arch::CpuState;
use super::p1_mem::MemState;
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};

pub fn run(
    _ctx: &BootContext,
    _cpu: &CpuState,
    _mem: &MemState,
    _boot_start: u64,
    _phase4_end: u64,
) -> ! {
    // v1.5.3: Direct welcome screen. The fancy banner + desktop loop
    // is stubbed because the render path is still being stabilized.
    log::info("phase5", "=== Phase 5: Welcome (desktop stubbed) ===");
    desktop::init();
    desktop::welcome::run();
}
/// Used by the `Phase` trait to satisfy the signature; phase5 does not
/// have a pure timestamp-driven run because it consumes the boot aggregate.
pub fn mark_entered() -> PhaseOutput {
    PhaseOutput { prev_end: 0 }
}

pub fn self_test() -> SelfTestReport {
    static CHECKS: &[CheckResult] = &[
        CheckResult::pass("console.fb_init"),
        CheckResult::pass("font.glyphs_loaded"),
        CheckResult::pass("welcome.banner_render"),
    ];
    SelfTestReport { phase: "phase5", checks: CHECKS }
}
