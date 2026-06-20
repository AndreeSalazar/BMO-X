#![allow(dead_code)]

//! Control Register (CR0/CR4) configuration for x86-64 Long Mode.

use super::features::CpuFeatures;
use core::arch::asm;

/// Configure CR0 and CR4 for optimal x86-64 operation.
///
/// Enables: FPU, SSE, AVX, OSXSAVE, WP, SMEP, SMAP, UMIP, FSGSBASE.
/// Disables: Emulation (EM), Task Switched (TS).
pub fn init(features: &CpuFeatures) {
    unsafe {
        // CR0: enable FPU, disable emulation, set WP
        let mut cr0: u64;
        asm!("mov {}, cr0", out(reg) cr0);
        cr0 |= 1 << 1;    // MP (Monitor Coprocessor)
        cr0 &= !(1 << 2); // clear EM (Emulation)
        cr0 |= 1 << 5;    // NE (Numeric Error)
        cr0 |= 1 << 16;   // WP (Write Protect)
        cr0 &= !(1 << 3); // clear TS (Task Switched — lazy FPU)
        asm!("mov cr0, {}", in(reg) cr0);

        // CR4: enable SSE, AVX, OSXSAVE, SMEP, SMAP, UMIP, FSGSBASE
        let mut cr4: u64;
        asm!("mov {}, cr4", out(reg) cr4);
        if features.has_sse {
            cr4 |= 1 << 9;  // OSFXSR
        }
        if features.has_sse2 {
            cr4 |= 1 << 10; // OSXMMEXCPT
        }
        if features.has_avx && features.has_osxsave {
            cr4 |= 1 << 18; // OSXSAVE
        }
        if features.has_fs_gs_base {
            cr4 |= 1 << 13; // FSGSBASE
        }
        if features.has_smep {
            cr4 |= 1 << 20; // SMEP
        }
        if features.has_smap {
            cr4 |= 1 << 21; // SMAP
        }
        if features.has_umip {
            cr4 |= 1 << 11; // UMIP
        }
        asm!("mov cr4, {}", in(reg) cr4);
    }
    crate::device::serial::serial_write("[cpu] CR0/CR4 configured\n");
}
