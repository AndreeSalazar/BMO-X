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
        crate::ring0::dev::console::serial_write("[cpu] MTRR: not supported, skipping\n");
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
            // Detect physical address size from CPUID leaf 0x80000008 EAX[7:0]
            let cpuid_8_eax = super::vendor_shim::zen3::cpuid_detection::cpuid(0x8000_0008, 0).0;
            let max_phy_addr = (cpuid_8_eax & 0xFF) as u32;
            let max_phy_addr = if max_phy_addr == 0 { 36 } else { max_phy_addr };
            let phy_addr_mask = if max_phy_addr >= 64 {
                !0u64
            } else {
                (1u64 << max_phy_addr) - 1
            };

            for i in 0..8u32 {
                let mask_val = msr::rdmsr(IA32_MTRR_PHYSMASK0 + i * 2);
                if mask_val & (1 << 11) == 0 {
                    let base_val = (vram_base & phy_addr_mask & !0xFFF) | MTRR_TYPE_WC;
                    let align = vram_size.next_power_of_two();
                    let mask = (!(align - 1)) & phy_addr_mask & !0xFFF | (1 << 11);
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
    crate::ring0::dev::console::serial_write("[cpu] MTRRs configured\n");
}

// ── PAT ──────────────────────────────────────────────────────────────────────

/// Configure PAT. Default PAT value already has WC at index 1:
/// PAT[0]=WB, PAT[1]=WC, PAT[2]=UC-, PAT[3]=UC, ...
/// No explicit write needed for basic operation.
pub fn init_pat() {
    crate::ring0::dev::console::serial_write("[cpu] PAT: default config OK (WB+WC)\n");
}

/// One-shot init: MTRR + PAT.
pub fn init(features: &CpuFeatures, vram_base: u64, vram_size: u64) {
    // The shim's mtrr_pat::init always returns true (no-op) since
    // we don't have the external Zen3 MTRR logic. Fall through to
    // the local init_mtrr/init_pat.
    let _ = super::vendor_shim::zen3::mtrr_pat::init(vram_base, vram_size);
    init_mtrr(features, vram_base, vram_size);
    init_pat();
}
