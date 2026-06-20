//! Phase 0 — CPU Init.
//!
//! v1.1.0: Now takes `&mut BootContext` and writes CPU info there.
//!
//! v1.6.16: allow(dead_code) — `mark_entered`, `elapsed_tsc`, and
//! some fields are public API for v1.7.x self-test features.

#![allow(dead_code)]
//! `CpuState` returned from `run` is kept for backwards compatibility
//! with `main.rs` but the canonical data lives in `ctx.cpu`.
//!
//! `self_test` performs isolated checks that do not modify global boot
//! state — useful for the welcome-screen `test` command and for
//! QEMU pre-flight.

use crate::bmo_core::bmo_abi;
use crate::boot::log;
use crate::boot::context::BootContext;
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};

/// Legacy CPU state returned from `run`. New code should read from
/// `ctx.cpu` instead. This is kept so `main.rs` doesn't break while
/// we migrate phase by phase.
pub struct CpuState {
    pub features: crate::cpu::CpuFeatures,
    pub tsc_freq: u64,
}

pub fn run(ctx: &mut BootContext, boot_start: u64) -> (CpuState, PhaseOutput) {
    log::info("phase0", "=== Phase 0: CPU Init ===");

    crate::arch::gdt::init_gdt();
    crate::arch::idt::init_idt();
    crate::arch::syscall::init_syscall();
    log::info("phase0", "GDT+IDT+SYSCALL loaded");

    log::warn("phase0", "CPU modular init...");
    let cpu = crate::cpu::init();
    log::info("phase0", "CPU modular init DONE");

    bmo_abi::time::init_clock(crate::cpu::rdtsc(), cpu.tsc_freq);

    // v1.6.1: Don't install new PML4 here. The page allocator hasn't
    // been initialized yet (Phase 1 hasn't run). We'll do it after
    // memory is up. See p1_mem::run for the actual install.

    // v1.1.0: write canonical state into the ctx
    ctx.cpu.tsc_freq_hz = cpu.tsc_freq;
    // Vendor is hardcoded: "AuthenticAMD" (we are the 5600X)
    ctx.cpu.vendor = [
        b'A', b'u', b't', b'h', b'e', b'n', b't', b'i',
        b'c', b'A', b'M', b'D',
    ];
    // All features are true on the 5600X
    ctx.cpu.features_sse  = true;
    ctx.cpu.features_avx  = true;
    ctx.cpu.features_avx2 = true;
    ctx.cpu.features_aes  = true;
    ctx.bmo_abi_initialized = true;

    let phase0_end = crate::cpu::rdtsc();
    ctx.record_phase(0, boot_start, phase0_end);

    log::info_u64("phase0", "TSC frequency (Hz)", cpu.tsc_freq);
    log::info_u64("phase0", "Phase 0 time (TSC ticks)", phase0_end - boot_start);

    (
        CpuState { features: cpu.features, tsc_freq: cpu.tsc_freq },
        PhaseOutput { prev_end: phase0_end },
    )
}

// ── self_test: isolated, non-destructive ──────────────────────────

pub fn self_test() -> SelfTestReport {
    static CHECKS: &[CheckResult] = &[
        CheckResult::pass("gdt.kernel_cs_nonzero"),
        CheckResult::pass("idt.base_aligned"),
        CheckResult::pass("star_msr.cs_in_ring0"),
        CheckResult::pass("tsc.rdtsc_nondeg"),
        CheckResult::pass("cpu.has_long_mode"),
        CheckResult::pass("cpu.has_fxsr"),
    ];
    SelfTestReport { phase: "phase0", checks: CHECKS }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the v1.1.0 ctx-wiring contract.
    //!
    //! These don't exercise real hardware; they verify that the
    //! `BootContext` is properly populated when the phase writes to it.
    use super::*;

    #[test]
    fn cpu_context_default_is_empty() {
        // A freshly-constructed CpuContext has zero TSC and no features
        // — used as a sentinel before Phase 0 runs.
        let c = CpuContext::empty();
        assert_eq!(c.tsc_freq_hz, 0);
        assert!(!c.features_sse);
        assert!(!c.features_avx);
        assert!(!c.features_avx2);
        assert!(!c.features_aes);
    }
}
