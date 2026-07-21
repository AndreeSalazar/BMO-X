//! Errata workarounds for the Ryzen 5 5600X (Zen 3).
//!
//! Recovers the legacy `errata_workarounds.rs` from the deleted
//! `crates_Personal/ring0/cpu_vendor_profile/.../errata_workarounds.rs`,
//! simplified for the minimal Ring 0 base: we only expose the
//! `apply_all()` function that runs the AMD-recommended mitigations
//! in sequence and logs the result on the serial console.
//!
//! Mitigations applied:
//! - IBRS / STIBP  (Spectre v2)        via IA32_SPEC_CTRL
//! - SSB disable   (Spectre v4)        via IA32_SPEC_CTRL.SSBD
//! - IBPB           (cross-process)    via IA32_PRED_CMD
//! - TSX disable   (MDS)               via IA32_TSX_CTRL
//!
//! References:
//! - AMD Whitepaper "Software Techniques for Managing Speculation
//!   on AMD Processors" (rev 4.10, 2020-06-15)
//! - AMD-SB-1007

use core::arch::asm;

const MSR_IA32_SPEC_CTRL: u32 = 0x0000_0048;
const MSR_IA32_PRED_CMD:  u32 = 0x0000_0049;
const MSR_IA32_TSX_CTRL:  u32 = 0x0000_0122;
const MSR_AMD_SPEC_CTRL:  u32 = 0xC001_0115;

const SPEC_CTRL_IBRS: u64 = 1 << 0;
const SPEC_CTRL_STIBP: u64 = 1 << 1;
const SPEC_CTRL_SSBD:  u64 = 1 << 2;
const PRED_CMD_IBPB:   u64 = 1 << 0;
const TSX_CTRL_RTM_DISABLE: u64 = 1 << 0;

#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32; let hi: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi,
         options(nostack, preserves_flags));
    ((hi as u64) << 32) | lo as u64
}

#[inline]
unsafe fn wrmsr(msr: u32, val: u64) {
    let lo = val as u32;
    let hi = (val >> 32) as u32;
    asm!("wrmsr", in("ecx") msr, in("eax") lo, in("edx") hi,
         options(nostack, preserves_flags));
}

/// CPUID leaf/subleaf, preserving RBX (LLVM reserves it).
unsafe fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (a, b, c, d): (u32, u32, u32, u32);
    core::arch::asm!(
        "push rbx",
        "cpuid",
        "mov {tmp:e}, ebx",
        "pop rbx",
        tmp = out(reg) b,
        inout("eax") leaf => a,
        inout("ecx") sub => c,
        out("edx") d,
        options(nostack, preserves_flags),
    );
    (a, b, c, d)
}

pub fn apply_all() -> u32 {
    // Mitigation result bitmask — bit i set = mitigation i applied
    //   0: IBRS  via IA32_SPEC_CTRL   3: IBPB via IA32_PRED_CMD
    //   1: STIBP via IA32_SPEC_CTRL   4: TSX  via IA32_TSX_CTRL
    //   2: SSBD  via IA32_SPEC_CTRL
    //
    // GOLDEN RULE: never touch an MSR the CPU does not enumerate — writing
    // an unimplemented MSR raises #GP. On real Zen 3 the old unconditional
    // TSX_CTRL (0x122, an Intel-only MSR; AMD has no TSX) reset the kernel.
    let mut applied: u32 = 0;

    // AMD enumerates speculation controls in CPUID 0x80000008 EBX:
    //   12 = IBPB, 14 = IBRS, 15 = STIBP, 24 = SSBD.
    let (_, ebx_ext, _, _) = unsafe { cpuid(0x8000_0008, 0) };
    let has_ibpb  = ebx_ext & (1 << 12) != 0;
    let has_ibrs  = ebx_ext & (1 << 14) != 0;
    let has_stibp = ebx_ext & (1 << 15) != 0;
    let has_ssbd  = ebx_ext & (1 << 24) != 0;

    if has_ibrs || has_stibp || has_ssbd {
        unsafe {
            let cur = rdmsr(MSR_IA32_SPEC_CTRL);
            let mut new = cur;
            if has_ibrs  { new |= SPEC_CTRL_IBRS; }
            if has_stibp { new |= SPEC_CTRL_STIBP; }
            if has_ssbd  { new |= SPEC_CTRL_SSBD; }
            wrmsr(MSR_IA32_SPEC_CTRL, new);
            let readback = rdmsr(MSR_IA32_SPEC_CTRL);
            if (readback & SPEC_CTRL_IBRS)  != 0 { applied |= 1 << 0; }
            if (readback & SPEC_CTRL_STIBP) != 0 { applied |= 1 << 1; }
            if (readback & SPEC_CTRL_SSBD)  != 0 { applied |= 1 << 2; }
        }
    }
    if has_ibpb {
        unsafe { wrmsr(MSR_IA32_PRED_CMD, PRED_CMD_IBPB); }
        applied |= 1 << 3;
    }

    // TSX_CTRL only exists on CPUs that enumerate RTM (CPUID.7.0 EBX[11]).
    // Zen 3 has no TSX, so this is skipped — which is the fix.
    let (_, ebx7, _, _) = unsafe { cpuid(7, 0) };
    if ebx7 & (1 << 11) != 0 {
        unsafe {
            wrmsr(MSR_IA32_TSX_CTRL, TSX_CTRL_RTM_DISABLE);
            let tsx = rdmsr(MSR_IA32_TSX_CTRL);
            if (tsx & TSX_CTRL_RTM_DISABLE) != 0 { applied |= 1 << 4; }
        }
    }

    applied
}
