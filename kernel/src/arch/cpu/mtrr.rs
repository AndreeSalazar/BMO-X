#![allow(dead_code)]

//! Memory Type Range Registers (MTRRs) — optimal memory mapping for RAM/VRAM.

use super::features::CpuFeatures;
use super::msrs::{self, IA32_MTRR_DEF_TYPE, IA32_MTRR_PHYSBASE0, IA32_MTRR_PHYSMASK0, MTRR_TYPE_WB, MTRR_TYPE_WC};

/// Configure MTRRs for optimal memory mapping.
///
/// Sets default to Write-Back (WB). If vram_size > 0, configures VRAM as
/// Write-Combining (WC) for better framebuffer performance.
pub fn init(features: &CpuFeatures, vram_base: u64, vram_size: u64) {
    if !features.has_mtrr {
        crate::drivers::serial::serial_write("[cpu] MTRR: not supported, skipping\n");
        return;
    }

    unsafe {
        // Disable MTRRs while configuring
        let def_type = msrs::rdmsr(IA32_MTRR_DEF_TYPE) & !0x800;
        msrs::wrmsr(IA32_MTRR_DEF_TYPE, def_type);

        // Set default memory type to Write-Back (WB)
        let mut def = msrs::rdmsr(IA32_MTRR_DEF_TYPE);
        def = (def & !0xFF) | MTRR_TYPE_WB;
        msrs::wrmsr(IA32_MTRR_DEF_TYPE, def);

        // Configure VRAM as Write-Combining (WC)
        if vram_size > 0 {
            for i in 0..8u32 {
                let mask_val = msrs::rdmsr(IA32_MTRR_PHYSMASK0 + i * 2);
                if mask_val & (1 << 11) == 0 {
                    let base_val = (vram_base & 0x000FFFFF_FFFFF000) | MTRR_TYPE_WC;
                    let align = vram_size.next_power_of_two();
                    let mask = (!(align - 1)) & 0x000FFFFF_FFFFF000 | (1 << 11);
                    msrs::wrmsr(IA32_MTRR_PHYSBASE0 + i * 2, base_val);
                    msrs::wrmsr(IA32_MTRR_PHYSMASK0 + i * 2, mask);
                    break;
                }
            }
        }

        // Re-enable MTRRs
        let mut def = msrs::rdmsr(IA32_MTRR_DEF_TYPE);
        def |= 0x800;
        msrs::wrmsr(IA32_MTRR_DEF_TYPE, def);

        // Flush caches and TLB
        unsafe {
            core::arch::asm!("wbinvd");
            let dummy: u64;
            core::arch::asm!("mov {}, cr3", out(reg) dummy);
            core::arch::asm!("mov cr3, {}", in(reg) dummy);
        }
    }
    crate::drivers::serial::serial_write("[cpu] MTRRs configured\n");
}
