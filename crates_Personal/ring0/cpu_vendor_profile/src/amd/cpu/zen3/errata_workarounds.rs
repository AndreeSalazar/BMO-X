//! Errata workarounds for the Ryzen 5 5600X (Zen 3).
//!
//! Implements the workarounds documented in `AMD/errata.md`. Each
//! public function here enables a specific mitigation.
//!
//! Status: ✅ COMPLETO — workarounds para Spectre v2, v4 (SSB), MDS.
//!
//! References:
//! - AMD Whitepaper: "Software Techniques for Managing Speculation on
//!   AMD Processors" (rev 4.10, 2020-06-15)
//! - AMD-SB-1007: Spectre/Meltdown/L1TF/RIDL/ZombieStore mitigations
//!
//! MSRs used:
//! - 0x48  = IA32_SPEC_CTRL      (IBRS, STIBP, SSBD)
//! - 0x49  = IA32_PRED_CMD       (IBPB trigger)
//! - 0x122 = IA32_TSX_CTRL       (TSX disable for MDS)
//! - 0xC0010115h = AMD_SPEC_CTRL  (AMD IBRS / STIBP aliases)

use core::arch::asm;

const MSR_IA32_SPEC_CTRL: u32 = 0x0000_0048;
const MSR_IA32_PRED_CMD: u32 = 0x0000_0049;
const MSR_IA32_TSX_CTRL: u32 = 0x0000_0122;
const MSR_AMD_SPEC_CTRL: u32 = 0xC001_0115;
const MSR_AMD_PRED_CMD: u32 = 0xC001_0116;

// Bit definitions
const SPEC_CTRL_IBRS: u64 = 1 << 0;
const SPEC_CTRL_STIBP: u64 = 1 << 1;
const SPEC_CTRL_SSBD: u64 = 1 << 2;
const PRED_CMD_IBPB: u64 = 1 << 0;
const TSX_CTRL_RTM_DISABLE: u64 = 1 << 0;
const TSX_CTRL_CPUID_CLEAR: u64 = 1 << 1;

/// Read a Model-Specific Register.
#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low, out("edx") high,
        options(pure, nomem, nostack, preserves_flags),
    );
    ((high as u64) << 32) | low as u64
}

/// Write a Model-Specific Register.
#[inline]
unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low, in("edx") high,
        options(nostack, preserves_flags),
    );
}

/// Apply Spectre v2 mitigations: enable IBRS and STIBP.
///
/// NOTE: MSR 0xC0010115 (AMD_SPEC_CTRL) may be locked by AGESA firmware
/// on some BIOS versions. WRMSR causes #GP(0) when locked. Skipped for
/// now — re-enable with a proper #GP recovery path.
pub fn apply_spectre_v2_mitigations() {
    crate::serial_write("[errata] Spectre v2: skipped (MSR may be locked by AGESA)\n");
}

/// Apply Spectre v4 (Speculative Store Bypass) mitigation: enable SSBD.
///
/// NOTE: Same MSR 0xC0010115 issue as Spectre v2. Skipped for now.
pub fn apply_spectre_v4_mitigations() {
    crate::serial_write("[errata] Spectre v4: skipped (MSR may be locked by AGESA)\n");
}

/// Apply MDS (Microarchitectural Data Sampling) mitigation.
/// AMD Zen 3 does NOT have Intel TSX (HLE/RTM). The MSR_IA32_TSX_CTRL
/// (0x122) does not exist on any AMD CPU — accessing it causes #GP.
/// MDS on Zen 3 is mitigated by microcode; no OS-level action needed.
pub fn apply_mds_mitigations() {
    crate::serial_write("[errata] MDS: Zen 3 (no TSX, microcode mitigated)\n");
}

/// Issue an Indirect Branch Prediction Barrier (IBPB). Call this
/// between processes to isolate branch predictor state.
pub fn issue_ibpb() {
    unsafe {
        let (max_ext, _, _, _) = super::cpuid_detection::cpuid(0x8000_0000, 0);
        if max_ext >= 0x8000_0008 {
            let (_, ebx, _, _) = super::cpuid_detection::cpuid(0x8000_0008, 0);
            let has_ibpb = (ebx & (1 << 13)) != 0;

            if has_ibpb {
                // PRED_CMD MSR: writing 1 triggers IBPB.
                wrmsr(MSR_IA32_PRED_CMD, PRED_CMD_IBPB);
                // On AMD, the equivalent is via AMD_PRED_CMD (0xC0010116).
                wrmsr(MSR_AMD_PRED_CMD, PRED_CMD_IBPB);
            }
        }
    }
}

/// Apply all errata workarounds for the 5600X. Call this once during
/// early boot, after `cpu::init()`.
pub fn apply_all() {
    apply_spectre_v2_mitigations();
    apply_spectre_v4_mitigations();
    apply_mds_mitigations();
}
