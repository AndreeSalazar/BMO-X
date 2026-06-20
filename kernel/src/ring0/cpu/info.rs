#![allow(dead_code)]

//! CPU information display — prints detected features to serial.

use super::features::CpuFeatures;

/// Print full CPU information to serial console.
pub fn print(features: &CpuFeatures) {
    crate::device::serial::serial_write("[cpu] === CPU Information ===\n");
    crate::device::serial::serial_write("[cpu] Brand: ");
    crate::device::serial::serial_write(features.brand_string_str());
    crate::device::serial::serial_write("\n");

    crate::device::serial::serial_write("[cpu] Family=");
    print_u32(features.cpu_family);
    crate::device::serial::serial_write(" Model=");
    print_u32(features.cpu_model);
    crate::device::serial::serial_write(" Stepping=");
    print_u32(features.cpu_stepping);
    crate::device::serial::serial_write("\n");

    crate::device::serial::serial_write("[cpu] Features: SSE");
    if features.has_sse2 { crate::device::serial::serial_write("/2"); }
    if features.has_sse3 { crate::device::serial::serial_write("/3"); }
    if features.has_ssse3 { crate::device::serial::serial_write("/SSSE3"); }
    if features.has_sse41 { crate::device::serial::serial_write("/4.1"); }
    if features.has_sse42 { crate::device::serial::serial_write("/4.2"); }
    if features.has_sse4a { crate::device::serial::serial_write("/4A"); }
    if features.has_avx { crate::device::serial::serial_write(" AVX"); }
    if features.has_avx2 { crate::device::serial::serial_write("2"); }
    if features.has_fma3 { crate::device::serial::serial_write(" FMA3"); }
    if features.has_aes { crate::device::serial::serial_write(" AES-NI"); }
    if features.has_sha { crate::device::serial::serial_write(" SHA"); }
    if features.has_bmi1 { crate::device::serial::serial_write(" BMI1"); }
    if features.has_bmi2 { crate::device::serial::serial_write("2"); }
    if features.has_popcnt { crate::device::serial::serial_write(" POPCNT"); }
    if features.has_lzcnt { crate::device::serial::serial_write(" LZCNT"); }
    if features.has_rdrand { crate::device::serial::serial_write(" RDRAND"); }
    if features.has_rdseed { crate::device::serial::serial_write(" RDSEED"); }
    crate::device::serial::serial_write("\n");

    crate::device::serial::serial_write("[cpu] Security: SMEP=");
    crate::device::serial::serial_write(if features.has_smep { "OK" } else { "--" });
    crate::device::serial::serial_write(" SMAP=");
    crate::device::serial::serial_write(if features.has_smap { "OK" } else { "--" });
    crate::device::serial::serial_write(" UMIP=");
    crate::device::serial::serial_write(if features.has_umip { "OK" } else { "--" });
    crate::device::serial::serial_write(" NX=");
    crate::device::serial::serial_write(if features.has_nx { "OK" } else { "--" });
    crate::device::serial::serial_write("\n");
}

fn print_u32(val: u32) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    let mut v = val;
    if v == 0 { i -= 1; buf[i] = b'0'; }
    else { while v > 0 { i -= 1; buf[i] = b'0' + (v % 10) as u8; v /= 10; } }
    crate::device::serial::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}
