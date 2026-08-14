//! **THE ZEN 3 PROFILE** -- everything true only of a Ryzen 5 5600X.
//!
//! === Why this is a small file, and should stay small ===
//!
//! Because every line in here is a line that a different CPU would need
//! replaced: the AMD `EFER`/`SYSCFG`/`HWCR` bits, the Zen 3 performance
//! counters, and the TSC calibration around a 3,7 GHz base with a 4,6 GHz
//! boost.
//!
//! ** Its size is a measurement of how much of BMO-X is tied to one machine.
//! Right now it is 55 lines, and the honest reading of that number is: **the
//! boot stage is nearly portable, and was only written as though it were not.**
//!
//! Adding a CPU is a sibling of this file plus a line in `detect.rs`. It is not
//! a search through the boot stage.

#[allow(unused_imports)]
use crate::*;

// ===================================================================
//  AMD ZEN 3 EFER / SYSCFG / HARDWARE CONFIG
// ===================================================================

pub unsafe fn init_amd_msrs() {
    // ONLY the MSR writes actually required to boot. Early boot must not
    // poke model-specific or reserved MSR bits: on real Zen 3 those #GP
    // where QEMU silently accepts them (the exact reason this reset on
    // hardware but not in the emulator).
    //
    // EFER: SYSCALL enable (SCE, for the SYSCALL instruction) + NXE (so
    // page tables may set the No-Execute bit). Both are architectural and
    // required. LMA is already set (we are in long mode).
    let efer = rdmsr(MSR_EFER);
    let new_efer = efer | EFER_SCE | EFER_NXE;
    wrmsr(MSR_EFER, new_efer);
    ser_print!("[s1_cpu] EFER: 0x");
    ser_hex!(new_efer);
    ser_print!("\n");

    // Deliberately NOT written here (all optional / hazardous early):
    //   * SYSCFG bit 25 -- FSGSBASE is enabled via CR4.FSGSBASE (done in
    //     init_cr0_cr4); that SYSCFG bit is reserved and writing it #GPs.
    //   * HWCR flush-filter -- a micro perf tweak, not needed to boot.
}

pub fn cpu_has_sme() -> bool { unsafe { CPU.has_sme } }


// ===================================================================
//  TSC CALIBRATION (Zen 3: 3.7 GHz base, 4.6 GHz boost)
// ===================================================================

pub fn calibrate_tsc() -> u64 {
    // AMD Zen 3 has CPUID 0x15 with:
    //   EAX = TSC/crystal ratio denominator
    //   EBX = TSC/crystal ratio numerator
    //   ECX = crystal frequency in Hz
    // TSC_freq = ECX * EBX / EAX
    let (eax, ebx, ecx, _) = cpuid(0x15, 0);
    if eax != 0 && ebx != 0 && ecx != 0 {
        (ecx as u64) * (ebx as u64) / (eax as u64)
    } else {
        // Fallback: 5600X runs at 3.7 GHz base
        3_700_000_000
    }
}

pub unsafe fn init_tsc() {
    let freq = calibrate_tsc();
    CPU.tsc_freq = freq;
    ser_print!("[s1_cpu] TSC: ");
    ser_dec!(freq as usize);
    ser_print!(" Hz\n");
}
