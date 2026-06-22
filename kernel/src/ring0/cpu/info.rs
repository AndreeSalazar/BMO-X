//! CPU information display for the Ryzen 5 5600X.
//!
//! v1.8.7: ya no importa `features` (las constantes `HAS_*` se eliminaron
//! porque no se consumían). Solo se usa `cpuid` para leer el brand string.

#![allow(dead_code)]

use super::cpuid;

/// Print CPU info to serial console.
pub fn print() {
    crate::dev::console::serial_write("[cpu] === CPU Information (Ryzen 5 5600X) ===\n");

    // Brand string from CPUID 0x8000_0002-4
    let (max_ext, _, _, _) = cpuid(0x8000_0000, 0);
    if max_ext >= 0x8000_0004 {
        let mut brand = [0u8; 48];
        for i in 0..3 {
            let (a, b, c, d) = cpuid(0x8000_0002 + i, 0);
            let off = i as usize * 16;
            brand[off..off + 4].copy_from_slice(&a.to_le_bytes());
            brand[off + 4..off + 8].copy_from_slice(&b.to_le_bytes());
            brand[off + 8..off + 12].copy_from_slice(&c.to_le_bytes());
            brand[off + 12..off + 16].copy_from_slice(&d.to_le_bytes());
        }
        let len = brand.iter().position(|&b| b == 0).unwrap_or(brand.len());
        crate::dev::console::serial_write("[cpu] Brand: ");
        crate::dev::console::serial_write(
            core::str::from_utf8(&brand[..len]).unwrap_or("(invalid)"),
        );
        crate::dev::console::serial_write("\n");
    }

    // Family / model / stepping
    let (eax, _, _, _) = cpuid(1, 0);
    let stepping = eax & 0xF;
    let base_model = (eax >> 4) & 0xF;
    let base_family = (eax >> 8) & 0xF;
    let family = if base_family == 0xF { base_family + ((eax >> 20) & 0xFF) } else { base_family };
    let model = if family >= 0x6 { base_model | ((eax >> 12) & 0xF0) } else { base_model };
    crate::dev::console::serial_write("[cpu] Family=0x");
    print_hex(family);
    crate::dev::console::serial_write(" Model=0x");
    print_hex(model);
    crate::dev::console::serial_write(" Stepping=0x");
    print_hex(stepping);
    crate::dev::console::serial_write("\n");

    // Features: we just say "Zen 3" and that everything is true.
    crate::dev::console::serial_write("[cpu] Zen 3 (Vermeer), all features supported\n");

    // Show what's NOT supported (the only things that are not "true").
    // 5600X lacks AVX-512 and 5-level paging (LA57).
    crate::dev::console::serial_write("[cpu] Not supported: AVX-512, LA57 (5-level paging)\n");
}

fn print_hex(val: u32) {
    let hex = b"0123456789ABCDEF";
    let mut buf = [0u8; 8];
    for i in 0..8 {
        let nib = ((val >> (28 - i * 4)) & 0xF) as usize;
        buf[i] = hex[nib];
    }
    crate::dev::console::serial_write(
        core::str::from_utf8(&buf).unwrap_or("00000000"),
    );
}
