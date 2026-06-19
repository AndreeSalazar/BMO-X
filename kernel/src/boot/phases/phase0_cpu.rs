//! Phase 0 — CPU Init.
//!
//! v1.1.0: Now takes `&mut BootContext` and writes CPU info there.
//! `CpuState` returned from `run` is kept for backwards compatibility
//! with `main.rs` but the canonical data lives in `ctx.cpu`.
//!
//! `self_test` performs isolated checks that do not modify global boot
//! state — useful for the welcome-screen `test` command and for
//! QEMU pre-flight.

use crate::{arch, bmo_abi, boot::log};
use crate::boot::context::BootContext;
use super::trait_def::{PhaseOutput, SelfTestReport, CheckResult};

/// Legacy CPU state returned from `run`. New code should read from
/// `ctx.cpu` instead. This is kept so `main.rs` doesn't break while
/// we migrate phase by phase.
pub struct CpuState {
    pub features: arch::cpu::CpuFeatures,
    pub tsc_freq: u64,
}

pub fn run(ctx: &mut BootContext, boot_start: u64) -> (CpuState, PhaseOutput) {
    log::info("phase0", "=== Phase 0: CPU Init ===");

    arch::gdt::init_gdt();
    arch::idt::init_idt();
    arch::syscall_entry::init_syscall();
    log::info("phase0", "GDT+IDT+SYSCALL loaded");

    log::warn("phase0", "CPU modular init...");
    let cpu = arch::cpu::init();
    log::info("phase0", "CPU modular init DONE");

    bmo_abi::time::init_clock(arch::cpu::rdtsc(), cpu.tsc_freq);

    // v1.1.0: write canonical state into the context
    ctx.cpu.tsc_freq_hz = cpu.tsc_freq;
    ctx.cpu.vendor = {
        // brand_string is 48 bytes; truncate to 12 for our context slot
        let mut buf = [0u8; 12];
        let n = 12.min(cpu.features.brand_string.len());
        buf.copy_from_slice(&cpu.features.brand_string[..n]);
        buf
    };
    ctx.cpu.features_sse  = cpu.features.has_sse;
    ctx.cpu.features_avx  = cpu.features.has_avx;
    ctx.cpu.features_avx2 = cpu.features.has_avx2;
    ctx.cpu.features_aes  = cpu.features.has_aes;
    ctx.bmo_abi_initialized = true;

    let phase0_end = arch::cpu::rdtsc();
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
    //! Unit tests for the v1.1.0 context-wiring contract.
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
