#![allow(dead_code)]

//! Performance monitoring counters — fixed counter 0 (instructions retired).
//!
//! v1.8.7: `instructions_retired` se eliminó (sin consumidores cross-layer).
//! Si en el futuro quieres exponer métricas, restaurarla con la firma:
//!   `pub fn instructions_retired() -> u64` leyendo IA32_FIXED_CTR0 vía MSR.

use super::features::CpuFeatures;
use super::msr::{IA32_PERF_GLOBAL_CTRL, IA32_FIXED_CTR0};

/// Initialize performance monitoring counters.
///
/// Only enables fixed counter 0 (instructions retired) if the CPU supports it.
/// Safe: checks CPUID before writing MSRs.
pub fn init(features: &CpuFeatures) {
    if !features.has_perfctr_core {
        crate::dev::console::serial_write("[cpu] Perf counters: not supported, skipping\n");
        return;
    }

    unsafe {
        // Disable all counters first
        crate::cpu::msr::wrmsr(IA32_PERF_GLOBAL_CTRL, 0);

        // Reset fixed counter 0
        crate::cpu::msr::wrmsr(IA32_FIXED_CTR0, 0);

        // Enable fixed counter 0
        let mut ctrl = crate::cpu::msr::rdmsr(IA32_PERF_GLOBAL_CTRL);
        ctrl |= 1u64 << 32; // EN_FIXED_CTR0
        crate::cpu::msr::wrmsr(IA32_PERF_GLOBAL_CTRL, ctrl);
    }
    crate::dev::console::serial_write("[cpu] Perf counters initialized\n");
}
