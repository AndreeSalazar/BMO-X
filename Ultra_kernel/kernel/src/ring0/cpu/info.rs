//! CPU information display.
//!
//! In Ultra_kernel we don't depend on the legacy `cpu_vendor_profile`
//! crate. We use the local shim's `CpuIdentity` (raw CPUID output) and
//! print a compact summary.

use super::cpuid;
use super::vendor_shim::zen3;

pub fn print() {
    crate::ring0::dev::console::serial_write("[cpu] === CPU Information ===\n");

    // Vendor string
    let (eax_max, ebx, ecx, edx) = cpuid(0, 0);
    if eax_max >= 1 {
        let mut vendor = [0u8; 12];
        vendor[0..4].copy_from_slice(&ebx.to_ne_bytes());
        vendor[4..8].copy_from_slice(&edx.to_ne_bytes());
        vendor[8..12].copy_from_slice(&ecx.to_ne_bytes());
        crate::ring0::dev::console::serial_write("[cpu] Vendor: ");
        if let Ok(s) = core::str::from_utf8(&vendor) {
            crate::ring0::dev::console::serial_write(s.trim_end_matches('\0'));
        }
        crate::ring0::dev::console::serial_write("\n");
    }

    // Brand string (leaf 0x80000002..04)
    if eax_max >= 0x80000000 {
        let (a, b, c, d) = cpuid(0x80000002, 0);
        let (e, f, g, h) = cpuid(0x80000003, 0);
        let (i, j, k, l) = cpuid(0x80000004, 0);
        let mut buf = [0u8; 48];
        let mut idx = 0;
        for v in [a, b, c, d, e, f, g, h, i, j, k, l] {
            if idx < 48 { buf[idx] = v as u8; idx += 1; }
            if v > 0xFF && idx < 48 { buf[idx] = (v >> 8) as u8; idx += 1; }
            if v > 0xFFFF && idx < 48 { buf[idx] = (v >> 16) as u8; idx += 1; }
            if v > 0xFFFFFF && idx < 48 { buf[idx] = (v >> 24) as u8; idx += 1; }
        }
        crate::ring0::dev::console::serial_write("[cpu] Brand: ");
        if let Ok(s) = core::str::from_utf8(&buf[..idx.min(48)]) {
            crate::ring0::dev::console::serial_write(s.trim_end_matches('\0').trim_end());
        }
        crate::ring0::dev::console::serial_write("\n");
    }

    // Family/model/stepping
    if eax_max >= 1 {
        let (eax1, _, _, _) = cpuid(1, 0);
        let family = (eax1 >> 8) & 0xF;
        let model = (eax1 >> 4) & 0xF;
        let stepping = eax1 & 0xF;
        crate::ring0::dev::console::serial_write("[cpu] Family 0x");
        crate::ring0::dev::console::serial_write_u64(family as u64, 16);
        crate::ring0::dev::console::serial_write("h, Model 0x");
        crate::ring0::dev::console::serial_write_u64(model as u64, 16);
        crate::ring0::dev::console::serial_write("h, Stepping 0x");
        crate::ring0::dev::console::serial_write_u64(stepping as u64, 16);
        crate::ring0::dev::console::serial_write("h\n");
    }

    // TSC frequency
    let tsc_freq = zen3::tsc_freq_hz();
    if tsc_freq > 0 {
        crate::ring0::dev::console::serial_write("[cpu] TSC: ");
        crate::ring0::dev::console::serial_write_u64(tsc_freq, 10);
        crate::ring0::dev::console::serial_write(" Hz\n");
    }

    // Feature bits
    crate::ring0::dev::console::serial_write("[cpu] Features: ");
    if let Some(id) = zen3::cpuid_detection::identity() {
        if zen3::cpuid_detection::has_smep(&id) { crate::ring0::dev::console::serial_write("SMEP "); }
        if zen3::cpuid_detection::has_smap(&id) { crate::ring0::dev::console::serial_write("SMAP "); }
        if zen3::cpuid_detection::has_fsgsbase(&id) { crate::ring0::dev::console::serial_write("FSGSBASE "); }
        if zen3::cpuid_detection::has_avx(&id) { crate::ring0::dev::console::serial_write("AVX "); }
        if zen3::cpuid_detection::has_avx2(&id) { crate::ring0::dev::console::serial_write("AVX2 "); }
        if id.features_edx & (1 << 4) != 0 { crate::ring0::dev::console::serial_write("TSC "); }
        if id.features_edx & (1 << 27) != 0 { crate::ring0::dev::console::serial_write("RDTSCP "); }
    }
    crate::ring0::dev::console::serial_write("\n");
}
