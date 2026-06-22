#![allow(dead_code)]

//! Control Register (CR0/CR2/CR3/CR4/CR8) and Extended Control Register
//! (XCR0) helpers.
//!
//! Encapsulates `mov crN, ...` and `xsetbv` semantics with safety
//! checks (OSXSAVE must be set before xsetbv; XCR0 value must be valid).
//!
//! v1.8.7: `read_cr3` y `write_cr3` quedaron duplicadas con
//! `mem::virt::read_cr3` / `mem::virt::write_cr3`. Las versiones de
//! `mem::virt` son las canónicas (las usa `proc::mod::schedule`).
//! Aquí solo se mantienen los read de CR0/CR2/CR4/CR8 que necesita
//! `init` y `init_xcr0`. Si en el futuro algún consumidor externo
//! pide `regs::read_cr3`, re-exponerla con `pub use mem::virt::read_cr3`.

use super::features::CpuFeatures;
use core::arch::asm;

// ── CR0/CR2/CR4/CR8 read helpers (uso interno de `init`/`init_xcr0`) ────────

#[inline]
fn read_cr0() -> u64 {
    let v: u64;
    unsafe { asm!("mov {}, cr0", out(reg) v) };
    v
}

#[inline]
fn read_cr2() -> u64 {
    let v: u64;
    unsafe { asm!("mov {}, cr2", out(reg) v) };
    v
}

#[inline]
fn read_cr4() -> u64 {
    let v: u64;
    unsafe { asm!("mov {}, cr4", out(reg) v) };
    v
}

#[inline]
fn read_cr8() -> u64 {
    let v: u64;
    unsafe { asm!("mov {}, cr8", out(reg) v) };
    v
}

// ── CR0/CR4 feature gating ──────────────────────────────────────────────────

/// Configure CR0 and CR4 for optimal x86-64 operation.
///
/// Enables: FPU, SSE, AVX, OSXSAVE, WP, SMEP, SMAP, UMIP, FSGSBASE.
/// Disables: Emulation (EM), Task Switched (TS).
pub fn init(features: &CpuFeatures) {
    unsafe {
        // CR0: enable FPU, disable emulation, set WP
        let mut cr0 = read_cr0();
        cr0 |= 1 << 1;    // MP (Monitor Coprocessor)
        cr0 &= !(1 << 2); // clear EM (Emulation)
        cr0 |= 1 << 5;    // NE (Numeric Error)
        cr0 |= 1 << 16;   // WP (Write Protect)
        cr0 &= !(1 << 3); // clear TS (Task Switched — lazy FPU)
        asm!("mov cr0, {}", in(reg) cr0);

        // CR4: enable SSE, AVX, OSXSAVE, SMEP, SMAP, UMIP, FSGSBASE
        let mut cr4 = read_cr4();
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
    crate::dev::console::serial_write("[cpu] CR0/CR4 configured\n");
}

// ── XCR0 ────────────────────────────────────────────────────────────────────

/// Safely configure XCR0 for x87 + SSE + AVX state management.
///
/// Returns true on success, false if XCR0 could not be set (CR4.OSXSAVE
/// not set, or XCR0 value rejected by the hardware).
pub fn init_xcr0(features: &CpuFeatures) -> bool {
    if !features.has_avx || !features.has_osxsave {
        crate::dev::console::serial_write("[cpu] XCR0: skipped (no AVX/OSXSAVE)\n");
        return false;
    }

    unsafe {
        if read_cr4() & (1 << 18) == 0 {
            crate::dev::console::serial_write("[cpu] XCR0: FAIL — CR4.OSXSAVE not set!\n");
            return false;
        }

        // XCR0 = x87 (bit 0) | SSE (bit 1) | AVX (bit 2) = 7
        let xcr0_value: u64 = (1 << 0) | (1 << 1) | (1 << 2);
        let eax = (xcr0_value & 0xFFFFFFFF) as u32;
        let edx = (xcr0_value >> 32) as u32;

        asm!(
            "xsetbv",
            in("ecx") 0u32,
            in("eax") eax,
            in("edx") edx,
        );
    }
    crate::dev::console::serial_write("[cpu] XCR0 configured (x87 + SSE + AVX)\n");
    true
}
