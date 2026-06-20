#![allow(dead_code)]

//! CPU information display — prints detected features to serial.

use super::features::CpuFeatures;

/// Print full CPU information to serial console.
pub fn print(features: &CpuFeatures) {
    crate::dev::console::serial_write("[cpu] === CPU Information ===\n");
    crate::dev::console::serial_write("[cpu] Brand: ");
    crate::dev::console::serial_write(features.brand_string_str());
    crate::dev::console::serial_write("\n");

    crate::dev::console::serial_write("[cpu] Family=");
    print_u32(features.cpu_family);
    crate::dev::console::serial_write(" Model=");
    print_u32(features.cpu_model);
    crate::dev::console::serial_write(" Stepping=");
    print_u32(features.cpu_stepping);
    crate::dev::console::serial_write("\n");

    crate::dev::console::serial_write("[cpu] Features: SSE");
    if features.has_sse2 { crate::dev::console::serial_write("/2"); }
    if features.has_sse3 { crate::dev::console::serial_write("/3"); }
    if features.has_ssse3 { crate::dev::console::serial_write("/SSSE3"); }
    if features.has_sse41 { crate::dev::console::serial_write("/4.1"); }
    if features.has_sse42 { crate::dev::console::serial_write("/4.2"); }
    if features.has_sse4a { crate::dev::console::serial_write("/4A"); }
    if features.has_avx { crate::dev::console::serial_write(" AVX"); }
    if features.has_avx2 { crate::dev::console::serial_write("2"); }
    if features.has_fma3 { crate::dev::console::serial_write(" FMA3"); }
    if features.has_aes { crate::dev::console::serial_write(" AES-NI"); }
    if features.has_sha { crate::dev::console::serial_write(" SHA"); }
    if features.has_bmi1 { crate::dev::console::serial_write(" BMI1"); }
    if features.has_bmi2 { crate::dev::console::serial_write("2"); }
    if features.has_popcnt { crate::dev::console::serial_write(" POPCNT"); }
    if features.has_lzcnt { crate::dev::console::serial_write(" LZCNT"); }
    if features.has_rdrand { crate::dev::console::serial_write(" RDRAND"); }
    if features.has_rdseed { crate::dev::console::serial_write(" RDSEED"); }
    crate::dev::console::serial_write("\n");

    crate::dev::console::serial_write("[cpu] Security: SMEP=");
    crate::dev::console::serial_write(if features.has_smep { "OK" } else { "--" });
    crate::dev::console::serial_write(" SMAP=");
    crate::dev::console::serial_write(if features.has_smap { "OK" } else { "--" });
    crate::dev::console::serial_write(" UMIP=");
    crate::dev::console::serial_write(if features.has_umip { "OK" } else { "--" });
    crate::dev::console::serial_write(" NX=");
    crate::dev::console::serial_write(if features.has_nx { "OK" } else { "--" });
    crate::dev::console::serial_write("\n");
}

fn print_u32(val: u32) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    let mut v = val;
    if v == 0 { i -= 1; buf[i] = b'0'; }
    else { while v > 0 { i -= 1; buf[i] = b'0' + (v % 10) as u8; v /= 10; } }
    crate::dev::console::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}
