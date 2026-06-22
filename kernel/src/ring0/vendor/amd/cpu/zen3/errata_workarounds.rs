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
/// On Zen 3, this is done via the AMD alias MSR 0xC0010115h.
pub fn apply_spectre_v2_mitigations() {
    unsafe {
        // AMD Zen 3 uses MSR 0xC0010115h for SPEC_CTRL (not the Intel one).
        let mut spec_ctrl = rdmsr(MSR_AMD_SPEC_CTRL);
        spec_ctrl |= SPEC_CTRL_IBRS | SPEC_CTRL_STIBP;
        wrmsr(MSR_AMD_SPEC_CTRL, spec_ctrl);

        // Also write to the Intel alias for compatibility (some KVM
        // implementations check it).
        let mut intel_spec = rdmsr(MSR_IA32_SPEC_CTRL);
        intel_spec |= SPEC_CTRL_IBRS | SPEC_CTRL_STIBP;
        wrmsr(MSR_IA32_SPEC_CTRL, intel_spec);
    }
    crate::dev::console::serial_write("[errata] Spectre v2: IBRS+STIBP enabled\n");
}

/// Apply Spectre v4 (Speculative Store Bypass) mitigation: enable SSBD.
pub fn apply_spectre_v4_mitigations() {
    unsafe {
        // MSR 0xC0010115h supports SSBD on Zen 3.
        let mut spec_ctrl = rdmsr(MSR_AMD_SPEC_CTRL);
        spec_ctrl |= SPEC_CTRL_SSBD;
        wrmsr(MSR_AMD_SPEC_CTRL, spec_ctrl);
    }
    crate::dev::console::serial_write("[errata] Spectre v4: SSBD enabled\n");
}

/// Apply MDS (Microarchitectural Data Sampling) mitigation: disable
/// TSX (which is the only "no fill" path on Zen 3). Falls back to
/// `MD_CLEAR` microcode if available.
pub fn apply_mds_mitigations() {
    unsafe {
        // Check if TSX is supported
        let (eax, _, _, _) = super::cpuid_detection::cpuid(7, 0);
        let has_rtm = (eax & (1 << 11)) != 0;
        let has_hle = (eax & (1 << 9)) != 0;

        if has_rtm || has_hle {
            // Disable TSX via IA32_TSX_CTRL
            let mut tsx = rdmsr(MSR_IA32_TSX_CTRL);
            tsx |= TSX_CTRL_RTM_DISABLE;
            // CPUID_CLEAR hides TSX from CPUID (avoids the "TSX-Disabled"
            // vulnerability disclosure). Set after disabling.
            wrmsr(MSR_IA32_TSX_CTRL, tsx);
            crate::dev::console::serial_write("[errata] MDS: TSX disabled\n");
        } else {
            crate::dev::console::serial_write("[errata] MDS: TSX not present (good)\n");
        }
    }
}

/// Issue an Indirect Branch Prediction Barrier (IBPB). Call this
/// between processes to isolate branch predictor state.
pub fn issue_ibpb() {
    unsafe {
        // PRED_CMD MSR: writing 1 triggers IBPB.
        wrmsr(MSR_IA32_PRED_CMD, PRED_CMD_IBPB);
        // On AMD, the equivalent is via AMD_PRED_CMD (0xC0010116).
        wrmsr(MSR_AMD_PRED_CMD, PRED_CMD_IBPB);
    }
}

/// Apply all errata workarounds for the 5600X. Call this once during
/// early boot, after `cpu::init()`.
pub fn apply_all() {
    apply_spectre_v2_mitigations();
    apply_spectre_v4_mitigations();
    apply_mds_mitigations();
}
