//! Local shim for AMD Zen3 vendor profile.
//!
//! In the legacy kernel, these functions came from the `cpu_vendor_profile`
//! external crate. In Ultra_kernel, the crate is not available, so we
//! provide a minimal local stub that exposes the same function names with
//! the same types. The implementations are either CPUID-direct or hardcoded
//! to the Ryzen 5 5600X target.
//!
//! This module is `pub` so that `cpu/{info, tsc, features, cache}.rs` can
//! keep their structure (calling into `crate::vendor::amd::cpu::zen3::*`)
//! without dragging in an external crate.

#![allow(dead_code)]

pub mod zen3 {
    use core::arch::asm;

    // ── CPUID raw wrapper ─────────────────────────────────────
    pub mod cpuid_detection {
        /// CpuIdentity — the bare minimum copy from the legacy crate.
        #[derive(Debug, Clone, Copy)]
        pub struct CpuIdentity {
            pub vendor: [u8; 12],
            pub family: u32,
            pub model: u32,
            pub stepping: u32,
            pub features_ebx: u32,
            pub features_ecx: u32,
            pub features_edx: u32,
            pub features_7_ebx: u32,
            pub features_7_ecx: u32,
            pub features_8_ebx: u32,
            pub features_8000_0001_ecx: u32,
            pub features_8000_0001_edx: u32,
        }

        static mut IDENTITY: Option<CpuIdentity> = None;

        pub fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
            let (eax, ebx, ecx, edx): (u32, u32, u32, u32);
            unsafe {
                asm!(
                    "push rbx",
                    "cpuid",
                    "mov {ebx_out:e}, ebx",
                    "pop rbx",
                    inout("eax") leaf => eax,
                    inout("ecx") sub => ecx,
                    ebx_out = out(reg) ebx,
                    out("edx") edx,
                );
            }
            (eax, ebx, ecx, edx)
        }

        pub fn identity() -> Option<CpuIdentity> {
            unsafe {
                if let Some(id) = IDENTITY { return Some(id); }
                let (a, b, c, d) = cpuid(0, 0);
                if a == 0 { return None; }
                let mut vendor = [0u8; 12];
                vendor[0..4].copy_from_slice(&b.to_ne_bytes());
                vendor[4..8].copy_from_slice(&d.to_ne_bytes());
                vendor[8..12].copy_from_slice(&c.to_ne_bytes());
                let (_eax1, _ebx1, ecx1, edx1) = cpuid(1, 0);
                let (eax7, ebx7, ecx7, _edx7) = cpuid(7, 0);
                let (eax8, ebx8, _ecx8, _edx8) = cpuid(0x8000_0001, 0);
                let id = CpuIdentity {
                    vendor,
                    family: (eax1 >> 8) & 0xF,
                    model: (eax1 >> 4) & 0xF,
                    stepping: eax1 & 0xF,
                    features_ebx: ebx7,
                    features_ecx: ecx1,
                    features_edx: edx1,
                    features_7_ebx: ebx7,
                    features_7_ecx: ecx7,
                    features_8_ebx: ebx8,
                    features_8000_0001_ecx: ecx8,
                    features_8000_0001_edx: edx8,
                };
                IDENTITY = Some(id);
                Some(id)
            }
        }

        // Feature flag accessors — match legacy crate's API surface.
        pub fn has_smep(id: &CpuIdentity) -> bool { id.features_7_ebx & (1 << 7) != 0 }
        pub fn has_smap(id: &CpuIdentity) -> bool { id.features_7_ebx & (1 << 20) != 0 }
        pub fn has_fsgsbase(id: &CpuIdentity) -> bool { id.features_7_ebx & (1 << 0) != 0 }
        pub fn has_sse2(id: &CpuIdentity) -> bool { id.features_edx & (1 << 26) != 0 }
        pub fn has_avx(id: &CpuIdentity) -> bool { id.features_ecx & (1 << 28) != 0 }
        pub fn has_avx2(id: &CpuIdentity) -> bool { id.features_7_ebx & (1 << 5) != 0 }
    }

    // ── TSC ──────────────────────────────────────────────────
    pub fn tsc_freq_hz() -> u64 {
        // Default for AMD Ryzen 5 5600X (the target hardware).
        // Replace with real calibration when PIT/HPET are wired up.
        3_700_000_000
    }

    pub fn tsc_source() -> Option<&'static str> {
        Some("cpu_vendor_profile::default")
    }

    // ── Cache topology stub ─────────────────────────────────
    pub fn cache() -> Option<&'static str> {
        // Legacy crate returned a complex struct; for the splash and
        // boot path we just need "yes, we have a cache". A bare string
        // is not the same type as legacy's struct, so we leave this
        // as None to keep callers simple (they all use `if let Some`).
        None
    }

    pub mod bmo_cpu {
        pub fn topology() -> Option<Topo> {
            Some(Topo { cores: 1, threads: 1 })
        }
        pub struct Topo { pub cores: u32, pub threads: u32 }
    }

    // ── MTRR/PAT ────────────────────────────────────────────
    pub mod mtrr_pat {
        /// Stub: in legacy this configured MTRR for the framebuffer VRAM.
        /// We return `true` (success) so the caller proceeds; without
        /// real MTRR setup, the framebuffer will use the default
        /// Write-Back policy, which is fine for our UEFI GOP region.
        pub fn init(_vram_base: u64, _vram_size: u64) -> bool {
            true
        }
    }

    // ── AMD MSR initialization ─────────────────────────────
    /// Stub: the legacy kernel set up vendor-specific MSRs (SVM,
    /// SPEC_CTRL, etc.) here. In the Ring 0 base we don't have a
    /// real vendor implementation, so this is a no-op.
    pub fn init_msrs(_real_entry: u64) {}
}
