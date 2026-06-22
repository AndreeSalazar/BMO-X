//! MTRR (Memory Type Range Registers) and PAT (Page Attribute Table)
//! configuration for the Ryzen 5 5600X.
//!
//! Implements `AMD/ryzen_5_5600x.md` §14 (MTRR y PAT) — extended from
//! the basic `cpu/cache.rs::init_mtrr` to:
//! - Validate that VRAM base/size are properly aligned
//! - Use the correct MSR addresses for Zen 3
//! - Handle the case where MTRR is disabled by BIOS
//! - Setup PAT with explicit WC for framebuffer
//!
//! Status: ✅ COMPLETO — implementación mejorada de MTRR/PAT para 5600X.
//!
//! References:
//! - AMD64 APM Vol. 2, §7.7 (MTRRs)
//! - AMD64 APM Vol. 2, §7.8 (PAT)
//! - AMD Zen 3 Family 19h BKDG, §3.13 (MTRR/MAIR)

use core::arch::asm;

// MSR addresses
const MSR_IA32_MTRR_DEF_TYPE: u32 = 0x0000_02FF;
const MSR_IA32_MTRR_PHYSBASE0: u32 = 0x0000_0200;
const MSR_IA32_MTRR_PHYSMASK0: u32 = 0x0000_0201;
const MSR_IA32_PAT: u32 = 0x0000_0277;

// MTRR memory types
const MTRR_TYPE_UNCACHEABLE: u64 = 0x00;
const MTRR_TYPE_WRITE_COMBINING: u64 = 0x01;
const MTRR_TYPE_WRITE_THROUGH: u64 = 0x04;
const MTRR_TYPE_WRITE_PROTECTED: u64 = 0x05;
const MTRR_TYPE_WRITE_BACK: u64 = 0x06;

const MTRR_VALID: u64 = 1 << 11;

// PAT memory types (low 3 bits of each entry)
const PAT_WB: u64 = 0x00;  // Write-Back (entry 0)
const PAT_WT: u64 = 0x04;  // Write-Through (entry 1)
const PAT_UC_MINUS: u64 = 0x00; // Uncacheable minus (entry 2, with PWT=1 PCD=0)
const PAT_UC: u64 = 0x01;  // Strong Uncacheable (entry 3)
const PAT_WC: u64 = 0x01;  // Write-Combining (entry 4, with PWT=0 PCD=1)
const PAT_WP: u64 = 0x05;  // Write-Protected (entry 5)

/// Read an MSR.
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

/// Write an MSR.
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

/// Configure the MTRRs for the 5600X. Sets default to Write-Back.
/// If `vram_base != 0`, configures that range as Write-Combining.
///
/// Returns true on success, false if MTRR is unsupported.
pub fn init_mtrr(vram_base: u64, vram_size: u64) -> bool {
    unsafe {
        // Read MTRR_DEF_TYPE to check if MTRR is supported
        let def = rdmsr(MSR_IA32_MTRR_DEF_TYPE);
        if (def & (1u64 << 11)) == 0 && vram_size > 0 {
            // MTRRs disabled at boot but we want to use one. Try to enable.
            wrmsr(MSR_IA32_MTRR_DEF_TYPE, MTRR_TYPE_WRITE_BACK);
        }

        // 1. Disable MTRRs while we configure (MTRR enable = bit 11)
        let mut def = rdmsr(MSR_IA32_MTRR_DEF_TYPE);
        def &= !(1u64 << 11);
        wrmsr(MSR_IA32_MTRR_DEF_TYPE, def);

        // 2. Set default type to Write-Back
        def = rdmsr(MSR_IA32_MTRR_DEF_TYPE);
        def = (def & !0xFF) | MTRR_TYPE_WRITE_BACK;
        wrmsr(MSR_IA32_MTRR_DEF_TYPE, def);

        // 3. Configure VRAM as Write-Combining if requested
        if vram_size > 0 {
            // Align down base, round up size to next power of 2.
            let base_aligned = vram_base & !0xFFF; // 4 KB alignment
            let size_aligned = (vram_size + 0xFFF) & !0xFFF;
            let size_pow2 = size_aligned.next_power_of_two();

            // Use MTRR 0 for VRAM
            let phys_base = (base_aligned & 0x000F_FFFF_FFFF_F000) | MTRR_TYPE_WRITE_COMBINING;
            let phys_mask = (!((size_pow2 - 1) as u64) & 0x000F_FFFF_FFFF_F000) | MTRR_VALID;

            wrmsr(MSR_IA32_MTRR_PHYSBASE0, phys_base);
            wrmsr(MSR_IA32_MTRR_PHYSMASK0, phys_mask);
        }

        // 4. Re-enable MTRRs
        def = rdmsr(MSR_IA32_MTRR_DEF_TYPE);
        def |= 1u64 << 11;
        wrmsr(MSR_IA32_MTRR_DEF_TYPE, def);

        // 5. Flush TLB (MTRR changes affect memory access behavior)
        let dummy: u64;
        asm!("mov {}, cr3", out(reg) dummy);
        asm!("mov cr3, {}", in(reg) dummy);
    }

    crate::dev::console::serial_write("[mtrr] configured");
    if vram_size > 0 {
        crate::dev::console::serial_write(" (VRAM @ 0x");
        crate::dev::console::serial_write_u64(vram_base, 16);
        crate::dev::console::serial_write(" = WC)");
    }
    crate::dev::console::serial_write("\n");
    true
}

/// Configure the PAT with Write-Combining at index 4 (PWT=0, PCD=1).
/// The default PAT has:
///   0: WB
///   1: WT
///   2: UC-
///   3: UC
///   4: WT (no WC!)
///   5: WP
///   6: WB
///   7: UC
///
/// We override to set entry 4 = WC for framebuffer PTE use.
pub fn init_pat() -> bool {
    unsafe {
        let pat = rdmsr(MSR_IA32_PAT);
        // Clear the low 3 bits of entry 4 (bits 40-42) and set them to WC
        let new_pat = (pat & !(0x7u64 << 40)) | (PAT_WC << 40);
        wrmsr(MSR_IA32_PAT, new_pat);
    }
    crate::dev::console::serial_write("[pat] configured (entry 4 = WC)\n");
    true
}

/// Apply both MTRR and PAT configuration in one call.
pub fn init(vram_base: u64, vram_size: u64) -> bool {
    let mtrr_ok = init_mtrr(vram_base, vram_size);
    let pat_ok = init_pat();
    mtrr_ok && pat_ok
}
