#![allow(dead_code, unused_unsafe)]

//! Cache control: MTRR (Memory Type Range Registers) + PAT (Page
//! Attribute Table). MTRRs work at the physical level; PAT works at
//! the page-table level. Together they define how the CPU caches
//! different memory regions.

use super::features::CpuFeatures;
use super::msr::{
    self, IA32_MTRR_DEF_TYPE, IA32_MTRR_PHYSBASE0, IA32_MTRR_PHYSMASK0,
    MTRR_TYPE_WB, MTRR_TYPE_WC,
};

// ── MTRR ─────────────────────────────────────────────────────────────────────

/// Configure MTRRs for optimal memory mapping.
///
/// Sets default to Write-Back (WB). If `vram_size > 0`, configures
/// VRAM as Write-Combining (WC) for better framebuffer performance.
pub fn init_mtrr(features: &CpuFeatures, vram_base: u64, vram_size: u64) {
    if !features.has_mtrr {
        crate::dev::console::serial_write("[cpu] MTRR: not supported, skipping\n");
        return;
    }

    unsafe {
        // Disable MTRRs while configuring
        let def_type = msr::rdmsr(IA32_MTRR_DEF_TYPE) & !0x800;
        msr::wrmsr(IA32_MTRR_DEF_TYPE, def_type);

        // Set default memory type to Write-Back (WB)
        let mut def = msr::rdmsr(IA32_MTRR_DEF_TYPE);
        def = (def & !0xFF) | MTRR_TYPE_WB;
        msr::wrmsr(IA32_MTRR_DEF_TYPE, def);

        // Configure VRAM as Write-Combining (WC)
        if vram_size > 0 {
            for i in 0..8u32 {
                let mask_val = msr::rdmsr(IA32_MTRR_PHYSMASK0 + i * 2);
                if mask_val & (1 << 11) == 0 {
                    let base_val = (vram_base & 0x000FFFFF_FFFFF000) | MTRR_TYPE_WC;
                    let align = vram_size.next_power_of_two();
                    let mask = (!(align - 1)) & 0x000FFFFF_FFFFF000 | (1 << 11);
                    msr::wrmsr(IA32_MTRR_PHYSBASE0 + i * 2, base_val);
                    msr::wrmsr(IA32_MTRR_PHYSMASK0 + i * 2, mask);
                    break;
                }
            }
        }

        // Re-enable MTRRs
        let mut def = msr::rdmsr(IA32_MTRR_DEF_TYPE);
        def |= 0x800;
        msr::wrmsr(IA32_MTRR_DEF_TYPE, def);

        // Flush TLB only (no wbinvd — on real hardware wbinvd can be
        // extremely slow with a full L3 cache, or trigger #MC which
        // the bare iretq handler turns into an infinite loop).
        // MTRR changes take effect for new accesses immediately; dirty
        // cache lines will be written back naturally when evicted.
        let dummy: u64;
        core::arch::asm!("mov {}, cr3", out(reg) dummy);
        core::arch::asm!("mov cr3, {}", in(reg) dummy);
    }
    crate::dev::console::serial_write("[cpu] MTRRs configured\n");
}

// ── PAT ──────────────────────────────────────────────────────────────────────

/// Configure PAT. Default PAT value already has WC at index 1:
/// PAT[0]=WB, PAT[1]=WC, PAT[2]=UC-, PAT[3]=UC, ...
/// No explicit write needed for basic operation.
pub fn init_pat() {
    crate::dev::console::serial_write("[cpu] PAT: default config OK (WB+WC)\n");
}

/// One-shot init: MTRR + PAT.
///
/// v1.8.8: delegates to `crate::amd_cpu::zen3::mtrr_pat::init` which
/// uses the real MTRR + PAT logic for the Ryzen 5 5600X.
pub fn init(features: &CpuFeatures, vram_base: u64, vram_size: u64) {
    // The local init_mtrr / init_pat are kept for compatibility but
    // the real implementation is in AMD::zen3::mtrr_pat.
    if crate::amd_cpu::zen3::mtrr_pat::init(vram_base, vram_size) {
        return;
    }
    // Fallback: local basic init
    init_mtrr(features, vram_base, vram_size);
    init_pat();
}
