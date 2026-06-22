//! MSR definitions for the Ryzen 5 5600X (Zen 3, Family 19h).
//!
//! Implements `AMD/ryzen_5_5600x.md` §10 (MSRs fundamentales).
//!
//! Status: ✅ COMPLETO — tabla de MSRs específicos del 5600X.
//!
//! References:
//! - AMD64 Architecture Programmer's Manual Vol. 2, Chapter 6
//! - AMD Zen 3 Family 19h BKDG, Chapter 3 (MSR definitions)

/// Read an MSR. Use this instead of inline `asm!` to keep callers clean.
#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    core::arch::asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low, out("edx") high,
        options(pure, nomem, nostack, preserves_flags),
    );
    ((high as u64) << 32) | low as u64
}

/// Write an MSR.
#[inline]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    core::arch::asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low, in("edx") high,
        options(nostack, preserves_flags),
    );
}

// ═══════════════════════════════════════════════════════════════════════
//  AMD Family 19h (Zen 3) specific MSR addresses
// ═══════════════════════════════════════════════════════════════════════

/// IA32_EFER (Extended Feature Enable Register). Common to all AMD64.
pub const MSR_IA32_EFER: u32 = 0xC000_0080;
/// IA32_STAR (Syscall Target Address, legacy mode selector).
pub const MSR_IA32_STAR: u32 = 0xC000_0081;
/// IA32_LSTAR (Long Mode Syscall Target Address — 64-bit syscall entry).
pub const MSR_IA32_LSTAR: u32 = 0xC000_0082;
/// IA32_CSTAR (Compatibility Mode Syscall Target — usually unused in 64-bit).
pub const MSR_IA32_CSTAR: u32 = 0xC000_0083;
/// IA32_FMASK (RFLAGS Mask for SYSCALL).
pub const MSR_IA32_FMASK: u32 = 0xC000_0084;
/// IA32_FS_BASE / IA32_GS_BASE / IA32_KERNEL_GS_BASE (thread-local storage).
pub const MSR_IA32_FS_BASE: u32 = 0xC000_0100;
pub const MSR_IA32_GS_BASE: u32 = 0xC000_0101;
pub const MSR_IA32_KERNEL_GS_BASE: u32 = 0xC000_0102;
/// IA32_TSC_AUX (TSC auxiliary value, returned in ECX by RDTSCP).
pub const MSR_IA32_TSC_AUX: u32 = 0xC000_0103;

/// IA32_TSC (low 32 bits only — use rdtsc() in Rust for 64-bit).
pub const MSR_IA32_TSC: u32 = 0x0000_0010;
/// IA32_TSC_DEADLINE (TSC value at which the APIC timer fires).
pub const MSR_IA32_TSC_DEADLINE: u32 = 0x0000_06E0;

/// IA32_SPEC_CTRL (Spectre v2 control — IBRS, STIBP, SSBD).
pub const MSR_IA32_SPEC_CTRL: u32 = 0x0000_0048;
/// IA32_PRED_CMD (IBPB trigger).
pub const MSR_IA32_PRED_CMD: u32 = 0x0000_0049;
/// AMD_SPEC_CTRL (alias used on AMD Zen 3).
pub const MSR_AMD_SPEC_CTRL: u32 = 0xC001_0115;
/// AMD_PRED_CMD (alias used on AMD Zen 3).
pub const MSR_AMD_PRED_CMD: u32 = 0xC001_0116;

/// IA32_PAT (Page Attribute Table).
pub const MSR_IA32_PAT: u32 = 0x0000_0277;
/// IA32_MTRR_DEF_TYPE.
pub const MSR_IA32_MTRR_DEF_TYPE: u32 = 0x0000_02FF;
/// IA32_MTRR_PHYSBASE0..7, IA32_MTRR_PHYSMASK0..7 (at +0x100 stride).
pub const MSR_IA32_MTRR_PHYSBASE0: u32 = 0x0000_0200;
pub const MSR_IA32_MTRR_PHYSMASK0: u32 = 0x0000_0201;

/// IA32_APIC_BASE.
pub const MSR_IA32_APIC_BASE: u32 = 0x0000_001B;
/// IA32_PERF_GLOBAL_CTRL (performance counter control).
pub const MSR_IA32_PERF_GLOBAL_CTRL: u32 = 0x0000_038F;
/// IA32_FIXED_CTR0 (instructions retired counter).
pub const MSR_IA32_FIXED_CTR0: u32 = 0x0000_0309;

/// AMD-specific MSRs.
pub const MSR_AMD_PATCH_LEVEL: u32 = 0x0000_008B;
pub const MSR_AMD_SMM_BASE: u32 = 0xC001_0111;
pub const MSR_AMD_IBS_OP: u32 = 0xC001_1030;  // Instruction-Based Sampling

// ═══════════════════════════════════════════════════════════════════════
//  Bit definitions
// ═══════════════════════════════════════════════════════════════════════

/// IA32_EFER bits.
pub mod efer {
    pub const SCE: u64 = 1 << 0;      // SYSCALL/SYSRET enable
    pub const LME: u64 = 1 << 8;      // Long Mode Enable
    pub const LMA: u64 = 1 << 10;     // Long Mode Active (read-only)
    pub const NXE: u64 = 1 << 11;     // No-Execute Enable
    pub const SVME: u64 = 1 << 12;    // Secure Virtual Machine Enable
    pub const LMSLE: u64 = 1 << 13;   // Long Mode Segment Limit Enable
    pub const FFXSR: u64 = 1 << 14;   // Fast FXSAVE/FXSTOR
    pub const TCE: u64 = 1 << 15;     // Translation Cache Extension
}

/// MTRR memory types.
pub mod mtrr_type {
    pub const UC: u64 = 0x00;     // Uncacheable
    pub const WC: u64 = 0x01;     // Write-Combining
    pub const WT: u64 = 0x04;     // Write-Through
    pub const WP: u64 = 0x05;     // Write-Protected
    pub const WB: u64 = 0x06;     // Write-Back
    pub const VALID: u64 = 1 << 11;
}

/// SPEC_CTRL bits.
pub mod spec_ctrl {
    pub const IBRS: u64 = 1 << 0;
    pub const STIBP: u64 = 1 << 1;
    pub const SSBD: u64 = 1 << 2;
}

/// MTRR register access helpers.
pub mod mtrr {
    use super::*;

    /// Get the i-th MTRR pair's PHYSBASE value.
    pub unsafe fn read_physbase(i: u32) -> u64 {
        rdmsr(MSR_IA32_MTRR_PHYSBASE0 + i * 2)
    }

    /// Get the i-th MTRR pair's PHYSMASK value.
    pub unsafe fn read_physmask(i: u32) -> u64 {
        rdmsr(MSR_IA32_MTRR_PHYSMASK0 + i * 2)
    }

    /// Set the i-th MTRR pair.
    pub unsafe fn write_pair(i: u32, base: u64, mask: u64) {
        wrmsr(MSR_IA32_MTRR_PHYSBASE0 + i * 2, base);
        wrmsr(MSR_IA32_MTRR_PHYSMASK0 + i * 2, mask);
    }
}
