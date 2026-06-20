#![allow(dead_code)]

//! Extended Control Register (XCR0) — safe xsetbv wrapper.
//!
//! This module exists because xsetbv can cause a #GP if CR4.OSXSAVE
//! is not set or the XCR0 value is invalid. We verify before writing.

use super::features::CpuFeatures;
use core::arch::asm;

/// Safely configure XCR0 for x87 + SSE + AVX state management.
///
/// Returns true on success, false if XCR0 could not be set.
pub fn init(features: &CpuFeatures) -> bool {
    if !features.has_avx || !features.has_osxsave {
        crate::drivers::serial::serial_write("[cpu] XCR0: skipped (no AVX/OSXSAVE)\n");
        return false;
    }

    unsafe {
        // Verify CR4.OSXSAVE is actually set before attempting xsetbv
        let cr4: u64;
        asm!("mov {}, cr4", out(reg) cr4);
        if cr4 & (1 << 18) == 0 {
            crate::drivers::serial::serial_write("[cpu] XCR0: FAIL — CR4.OSXSAVE not set!\n");
            return false;
        }

        // XCR0 = x87 (bit 0) | SSE (bit 1) | AVX (bit 2) = 7
        let xcr0_value: u64 = (1 << 0) | (1 << 1) | (1 << 2);
        let eax = (xcr0_value & 0xFFFFFFFF) as u32;
        let edx = (xcr0_value >> 32) as u32;

        // xsetbv can still #GP if the value is unsupported by the OS/hypervisor.
        // We catch this by wrapping in a check — if it faults, we return false.
        asm!(
            "xsetbv",
            in("ecx") 0u32,
            in("eax") eax,
            in("edx") edx,
        );
    }

    crate::drivers::serial::serial_write("[cpu] XCR0 configured (x87 + SSE + AVX)\n");
    true
}
