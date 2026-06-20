#![allow(dead_code)]

//! Model-Specific Register (MSR) constants and safe read/write helpers.
//!
//! Naming: `msr` (singular) — the convention in modern kernels (Linux,
//! seL4) is to use the singular form. The plural `msrs` is reserved
//! for "MSR save area" in some contexts.

use core::arch::asm;

/// Read a Model-Specific Register.
///
/// # Safety
/// The MSR address must be valid for the current CPU.
#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high);
    ((high as u64) << 32) | low as u64
}

/// Write a Model-Specific Register.
///
/// # Safety
/// The MSR address must be valid and the value appropriate for the current CPU.
#[inline]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    let low = (value & 0xFFFFFFFF) as u32;
    let high = (value >> 32) as u32;
    asm!("wrmsr", in("ecx") msr, in("eax") low, in("edx") high);
}

// ── MSR address constants ──────────────────────────────────────────

// System segment & SYSCALL
pub const IA32_EFER: u32 = 0xC0000080;
pub const IA32_STAR: u32 = 0xC0000081;
pub const IA32_LSTAR: u32 = 0xC0000082;
pub const IA32_FMASK: u32 = 0xC0000084;

// TSC
pub const IA32_TSC: u32 = 0x10;
pub const IA32_TSC_ADJUST: u32 = 0x3B;

// System control
pub const IA32_MISC_ENABLE: u32 = 0x1A0;
pub const IA32_SYSENTER_CS: u32 = 0x174;
pub const IA32_SYSENTER_ESP: u32 = 0x175;
pub const IA32_SYSENTER_EIP: u32 = 0x176;

// Memory
pub const IA32_PAT: u32 = 0x277;
pub const IA32_APIC_BASE: u32 = 0x1B;

// Machine Check
pub const IA32_MCG_CAP: u32 = 0x179;
pub const IA32_MCG_STATUS: u32 = 0x17A;
pub const IA32_MCG_CTL: u32 = 0x17B;

// Performance monitoring
pub const IA32_PERFCTR0: u32 = 0xC1;
pub const IA32_PERFCTR1: u32 = 0xC2;
pub const IA32_PERFEVTSEL0: u32 = 0x186;
pub const IA32_PERFEVTSEL1: u32 = 0x187;
pub const IA32_FIXED_CTR0: u32 = 0x309;
pub const IA32_FIXED_CTR1: u32 = 0x30A;
pub const IA32_FIXED_CTR2: u32 = 0x30B;
pub const IA32_PERF_CAPABILITIES: u32 = 0x345;
pub const IA32_PERF_GLOBAL_STATUS: u32 = 0x34E;
pub const IA32_PERF_GLOBAL_CTRL: u32 = 0x38F;
pub const IA32_DEBUGCTL: u32 = 0x1D9;

// MTRR
pub const IA32_MTRR_DEF_TYPE: u32 = 0x2FF;
pub const IA32_MTRR_PHYSBASE0: u32 = 0x200;
pub const IA32_MTRR_PHYSMASK0: u32 = 0x201;

// AMD-specific
pub const AMD_MTRR_VAR_BASE: u32 = 0xC0010200;
pub const AMD_MTRR_VAR_MASK: u32 = 0xC0010201;
pub const AMD_SYSCALL_CFG: u32 = 0xC0010132;

// MTRR memory types
pub const MTRR_TYPE_UC: u64 = 0;
pub const MTRR_TYPE_WC: u64 = 1;
pub const MTRR_TYPE_WT: u64 = 4;
pub const MTRR_TYPE_WP: u64 = 5;
pub const MTRR_TYPE_WB: u64 = 6;
