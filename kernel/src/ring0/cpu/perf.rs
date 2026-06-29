#![allow(dead_code)]

//! Performance monitoring counters.
//!
//! v1.8.8: habilita los 3 fixed counters del 5600X:
//!   - IA32_FIXED_CTR0: instructions retired
//!   - IA32_FIXED_CTR1: core cycles
//!   - IA32_FIXED_CTR2: reference cycles
//!
//! También emite el MSR IA32_FIXED_CTR_CTRL (0x38D) que es necesario
//! ANTES de GLOBAL_CTRL para activar los counters en Zen 3.

use super::features::CpuFeatures;
use super::msr::{IA32_PERF_GLOBAL_CTRL, IA32_FIXED_CTR0,
                IA32_FIXED_CTR1, IA32_FIXED_CTR2, IA32_FIXED_CTR_CTRL};

/// Initialize performance monitoring counters.
///
/// v1.8.8: enables all 3 fixed counters if the CPU supports them.
pub fn init(features: &CpuFeatures) {
    if !features.has_perfctr_core {
        crate::dev::console::serial_write("[cpu] Perf counters: not supported, skipping\n");
        return;
    }

    unsafe {
        // 1. Disable all counters first
        crate::cpu::msr::wrmsr(IA32_PERF_GLOBAL_CTRL, 0);

        // 2. Reset all 3 fixed counters
        crate::cpu::msr::wrmsr(IA32_FIXED_CTR0, 0);
        crate::cpu::msr::wrmsr(IA32_FIXED_CTR1, 0);
        crate::cpu::msr::wrmsr(IA32_FIXED_CTR2, 0);

        // 3. Configure IA32_FIXED_CTR_CTRL (0x38D) to enable all 3 counters.
        //    Bit layout per counter (Intel SDM Vol.3B §18.2.3):
        //    bit 0    = EN (enable)
        //    bit 1    = PMI (interrupt on overflow) — RESERVED on AMD, must be 0
        //    bit 2    = OS (count CPL=0)
        //    bit 3    = USR (count CPL>0)
        //    Bit 1 set to 1 on AMD causes #GP (red screen crash).
        // For CTR0: bits 0-3; CTR1: bits 4-7; CTR2: bits 8-11.
        let ctrl = 0b1011_1011_1011;  // EN + OS + USR (no PMI) for all 3
        crate::cpu::msr::wrmsr(IA32_FIXED_CTR_CTRL, ctrl);

        // 4. Enable all 3 counters in GLOBAL_CTRL
        //    Bits 32-34 of IA32_PERF_GLOBAL_CTRL are EN_FIXED_CTR0/1/2.
        let mut global = crate::cpu::msr::rdmsr(IA32_PERF_GLOBAL_CTRL);
        global |= 0b111u64 << 32;
        crate::cpu::msr::wrmsr(IA32_PERF_GLOBAL_CTRL, global);
    }
    crate::dev::console::serial_write("[cpu] Perf counters initialized (3 fixed: inst, cycles, ref-cycles)\n");
}
