//! CPU information display for the Ryzen 5 5600X.
//!
//! v1.8.8: now reads ALL data from `crate::vendor::amd::cpu::zen3::*` (the real
//! detection layer). If `init_fastos_cpu()` hasn't run yet, prints
//! only the brand string and a hint.

#![allow(dead_code)]

use super::cpuid;

/// Print CPU info to serial console. Uses data from the real detection
/// layer (AMD/zen3/cpuid_detection + topology + cache).
pub fn print() {
    crate::dev::console::serial_write("[cpu] === CPU Information (Ryzen 5 5600X) ===\n");

    // Try to get the real detected identity
    if let Some(id) = crate::vendor::amd::cpu::zen3::cpuid_detection::identity() {
        // Brand string (up to 48 chars, null-padded)
        crate::dev::console::serial_write("[cpu] Brand: ");
        crate::dev::console::serial_write(id.brand.as_str());
        crate::dev::console::serial_write("\n");

        // Family/Model/Stepping
        crate::dev::console::serial_write("[cpu] Family 0x");
        crate::dev::console::serial_write_u64(id.family_model.family as u64, 16);
        crate::dev::console::serial_write("h, Model 0x");
        crate::dev::console::serial_write_u64(id.family_model.model as u64, 16);
        crate::dev::console::serial_write("h, Stepping 0x");
        crate::dev::console::serial_write_u64(id.family_model.stepping as u64, 16);
        crate::dev::console::serial_write("h (");
        crate::dev::console::serial_write(id.family_model.name());
        crate::dev::console::serial_write(")\n");

        // Logical cores
        crate::dev::console::serial_write("[cpu] Logical cores: ");
        crate::dev::console::serial_write_u64(id.logical_cores as u64, 10);
        crate::dev::console::serial_write(", Initial APIC ID: ");
        crate::dev::console::serial_write_u64(id.initial_apic_id as u64, 10);
        crate::dev::console::serial_write("\n");

        // Cache topology
        if let Some(c) = crate::vendor::amd::cpu::zen3::cache() {
            if let Some(c) = c.l1d {
                crate::dev::console::serial_write("[cpu] Cache L1d: ");
                crate::dev::console::serial_write_u64(c.size_kb as u64, 10);
                crate::dev::console::serial_write(" KB, ");
                crate::dev::console::serial_write_u64(c.associativity as u64, 10);
                crate::dev::console::serial_write("-way, line=");
                crate::dev::console::serial_write_u64(c.line_size_bytes as u64, 10);
                crate::dev::console::serial_write(" B\n");
            }
            if let Some(c) = c.l1i {
                crate::dev::console::serial_write("[cpu] Cache L1i: ");
                crate::dev::console::serial_write_u64(c.size_kb as u64, 10);
                crate::dev::console::serial_write(" KB, ");
                crate::dev::console::serial_write_u64(c.associativity as u64, 10);
                crate::dev::console::serial_write("-way, line=");
                crate::dev::console::serial_write_u64(c.line_size_bytes as u64, 10);
                crate::dev::console::serial_write(" B\n");
            }
            if let Some(c) = c.l2 {
                crate::dev::console::serial_write("[cpu] Cache L2: ");
                crate::dev::console::serial_write_u64(c.size_kb as u64, 10);
                crate::dev::console::serial_write(" KB, ");
                crate::dev::console::serial_write_u64(c.associativity as u64, 10);
                crate::dev::console::serial_write("-way, line=");
                crate::dev::console::serial_write_u64(c.line_size_bytes as u64, 10);
                crate::dev::console::serial_write(" B\n");
            }
            if let Some(c) = c.l3 {
                crate::dev::console::serial_write("[cpu] Cache L3: ");
                crate::dev::console::serial_write_u64(c.size_kb as u64, 10);
                crate::dev::console::serial_write(" KB, ");
                crate::dev::console::serial_write_u64(c.associativity as u64, 10);
                crate::dev::console::serial_write("-way, shared by ");
                crate::dev::console::serial_write_u64(c.shared_threads as u64, 10);
                crate::dev::console::serial_write(" threads\n");
            }
        }

        // TSC frequency
        let tsc_freq = crate::vendor::amd::cpu::zen3::tsc_freq_hz();
        if tsc_freq > 0 {
            crate::dev::console::serial_write("[cpu] TSC: ");
            crate::dev::console::serial_write_u64(tsc_freq, 10);
            crate::dev::console::serial_write(" Hz");
            if let Some(src) = crate::vendor::amd::cpu::zen3::tsc_source() {
                crate::dev::console::serial_write(" (calibrated via ");
                crate::dev::console::serial_write(src.name());
                crate::dev::console::serial_write(")");
            }
            crate::dev::console::serial_write("\n");
        }

        // Topology
        if let Some(topo) = crate::vendor::amd::cpu::zen3::fastos_cpu::topology() {
            crate::dev::console::serial_write("[cpu] Topology: ");
            crate::dev::console::serial_write_u64(topo.total_cores as u64, 10);
            crate::dev::console::serial_write(" cores, ");
            crate::dev::console::serial_write_u64(topo.total_threads as u64, 10);
            crate::dev::console::serial_write(" threads, ");
            crate::dev::console::serial_write_u64(topo.total_ccxs as u64, 10);
            crate::dev::console::serial_write(" CCX, ");
            crate::dev::console::serial_write_u64(topo.total_ccds as u64, 10);
            crate::dev::console::serial_write(" CCD\n");
        }

        // Features
        crate::dev::console::serial_write("[cpu] Features:\n");
        if id.features_edx & (1 << 25) != 0 { crate::dev::console::serial_write("  SSE, SSE2, MMX, FXSR\n"); }
        if id.features_ecx & (1 << 28) != 0 { crate::dev::console::serial_write("  AVX, AVX2 (CPUID.7.EBX[5])\n"); }
        if id.features_ecx & (1 << 25) != 0 { crate::dev::console::serial_write("  AES-NI\n"); }
        if id.features_edx & (1 << 8) != 0 { crate::dev::console::serial_write("  Invariant TSC (warning: not on 5600X)\n"); }
        if crate::vendor::amd::cpu::zen3::cpuid_detection::has_smep(id) { crate::dev::console::serial_write("  SMEP (Supervisor Mode Execution Prevention)\n"); }
        if crate::vendor::amd::cpu::zen3::cpuid_detection::has_smap(id) { crate::dev::console::serial_write("  SMAP (Supervisor Mode Access Prevention)\n"); }
        if crate::vendor::amd::cpu::zen3::cpuid_detection::has_fsgsbase(id) { crate::dev::console::serial_write("  FSGSBASE (RDFSBASE/WRGSBASE)\n"); }
        if id.features_edx & (1 << 27) != 0 { crate::dev::console::serial_write("  RDTSCP\n"); }

        crate::dev::console::serial_write("[cpu] NOT supported (5600X):\n");
        crate::dev::console::serial_write("  AVX-512 (5600X is Zen 3, no AVX-512)\n");
        crate::dev::console::serial_write("  LA57 (5-level paging) — only on Zen 4+\n");
    } else {
        // Fallback: no detection yet
        let (max_ext, _, _, _) = cpuid(0x80000000, 0);
        let (a, b, c, d) = cpuid(0x80000002, 0);
        let (e, f, g, h) = cpuid(0x80000003, 0);
        let (i, j, k, l) = cpuid(0x80000004, 0);
        crate::dev::console::serial_write("[cpu] Brand: ");
        let mut buf = [0u8; 48];
        let mut idx = 0;
        for (a, b, c, d) in [(a, b, c, d), (e, f, g, h), (i, j, k, l)] {
            for v in &[a, b, c, d] {
                if idx < 48 {
                    buf[idx] = *v as u8;
                    idx += 1;
                    if *v > 0xFF { buf[idx] = (*v >> 8) as u8; idx += 1; }
                    if *v > 0xFFFF { buf[idx] = (*v >> 16) as u8; idx += 1; }
                    if *v > 0xFFFFFF { buf[idx] = (*v >> 24) as u8; idx += 1; }
                }
            }
        }
        if let Ok(s) = core::str::from_utf8(&buf[..idx.min(48)]) {
            crate::dev::console::serial_write(s);
        }
        crate::dev::console::serial_write("\n");
        crate::dev::console::serial_write("[cpu] (run init_fastos_cpu() for full info)\n");
        crate::dev::console::serial_write("[cpu] Max ext leaf: 0x");
        crate::dev::console::serial_write_u64(max_ext as u64, 16);
        crate::dev::console::serial_write("\n");
    }
}

fn print_hex(val: u32) {
    let hex = b"0123456789ABCDEF";
    let mut buf = [0u8; 8];
    for i in 0..8 {
        buf[i] = hex[((val >> (28 - i * 4)) & 0xF) as usize];
    }
    crate::dev::console::serial_write(core::str::from_utf8(&buf).unwrap_or("????????"));
}
