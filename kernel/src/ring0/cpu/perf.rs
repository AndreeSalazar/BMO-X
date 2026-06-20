#![allow(dead_code)]

//! Performance monitoring counters — fixed counter 0 (instructions retired).

use super::features::CpuFeatures;
use super::msrs::{self, IA32_PERF_GLOBAL_CTRL, IA32_FIXED_CTR0};

/// Initialize performance monitoring counters.
///
/// Only enables fixed counter 0 (instructions retired) if the CPU supports it.
/// Safe: checks CPUID before writing MSRs.
pub fn init(features: &CpuFeatures) {
    if !features.has_perfctr_core {
        crate::device::serial::serial_write("[cpu] Perf counters: not supported, skipping\n");
        return;
    }

    unsafe {
        // Disable all counters first
        msrs::wrmsr(IA32_PERF_GLOBAL_CTRL, 0);

        // Reset fixed counter 0
        msrs::wrmsr(IA32_FIXED_CTR0, 0);

        // Enable fixed counter 0
        let mut ctrl = msrs::rdmsr(IA32_PERF_GLOBAL_CTRL);
        ctrl |= 1u64 << 32; // EN_FIXED_CTR0
        msrs::wrmsr(IA32_PERF_GLOBAL_CTRL, ctrl);
    }
    crate::device::serial::serial_write("[cpu] Perf counters initialized\n");
}

/// Read instructions retired counter (fixed counter 0).
#[inline]
pub fn instructions_retired() -> u64 {
    unsafe { msrs::rdmsr(IA32_FIXED_CTR0) }
}
