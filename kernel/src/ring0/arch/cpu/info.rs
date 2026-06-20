#![allow(dead_code)]

//! CPU information display — prints detected features to serial.

use super::features::CpuFeatures;

/// Print full CPU information to serial console.
pub fn print(features: &CpuFeatures) {
    crate::drivers::serial::serial_write("[cpu] === CPU Information ===\n");
    crate::drivers::serial::serial_write("[cpu] Brand: ");
    crate::drivers::serial::serial_write(features.brand_string_str());
    crate::drivers::serial::serial_write("\n");

    crate::drivers::serial::serial_write("[cpu] Family=");
    print_u32(features.cpu_family);
    crate::drivers::serial::serial_write(" Model=");
    print_u32(features.cpu_model);
    crate::drivers::serial::serial_write(" Stepping=");
    print_u32(features.cpu_stepping);
    crate::drivers::serial::serial_write("\n");

    crate::drivers::serial::serial_write("[cpu] Features: SSE");
    if features.has_sse2 { crate::drivers::serial::serial_write("/2"); }
    if features.has_sse3 { crate::drivers::serial::serial_write("/3"); }
    if features.has_ssse3 { crate::drivers::serial::serial_write("/SSSE3"); }
    if features.has_sse41 { crate::drivers::serial::serial_write("/4.1"); }
    if features.has_sse42 { crate::drivers::serial::serial_write("/4.2"); }
    if features.has_sse4a { crate::drivers::serial::serial_write("/4A"); }
    if features.has_avx { crate::drivers::serial::serial_write(" AVX"); }
    if features.has_avx2 { crate::drivers::serial::serial_write("2"); }
    if features.has_fma3 { crate::drivers::serial::serial_write(" FMA3"); }
    if features.has_aes { crate::drivers::serial::serial_write(" AES-NI"); }
    if features.has_sha { crate::drivers::serial::serial_write(" SHA"); }
    if features.has_bmi1 { crate::drivers::serial::serial_write(" BMI1"); }
    if features.has_bmi2 { crate::drivers::serial::serial_write("2"); }
    if features.has_popcnt { crate::drivers::serial::serial_write(" POPCNT"); }
    if features.has_lzcnt { crate::drivers::serial::serial_write(" LZCNT"); }
    if features.has_rdrand { crate::drivers::serial::serial_write(" RDRAND"); }
    if features.has_rdseed { crate::drivers::serial::serial_write(" RDSEED"); }
    crate::drivers::serial::serial_write("\n");

    crate::drivers::serial::serial_write("[cpu] Security: SMEP=");
    crate::drivers::serial::serial_write(if features.has_smep { "OK" } else { "--" });
    crate::drivers::serial::serial_write(" SMAP=");
    crate::drivers::serial::serial_write(if features.has_smap { "OK" } else { "--" });
    crate::drivers::serial::serial_write(" UMIP=");
    crate::drivers::serial::serial_write(if features.has_umip { "OK" } else { "--" });
    crate::drivers::serial::serial_write(" NX=");
    crate::drivers::serial::serial_write(if features.has_nx { "OK" } else { "--" });
    crate::drivers::serial::serial_write("\n");
}

fn print_u32(val: u32) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    let mut v = val;
    if v == 0 { i -= 1; buf[i] = b'0'; }
    else { while v > 0 { i -= 1; buf[i] = b'0' + (v % 10) as u8; v /= 10; } }
    crate::drivers::serial::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}
