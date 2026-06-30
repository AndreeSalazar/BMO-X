//! `bmo_core::bef::loader::tests` — Tests del loader BEF.
//!
//! Valida que el loader BEF:
//! 1. Detecta el formato correcto (BEF/PE/ELF).
//! 2. Parsea el header nativo correctamente.
//! 3. Genera una `Image` válida con entry point.
//! 4. Maneja los 3 formatos como "devorados".
//!
//! Los tests compilan programas reales (BMO) y los cargan.

#![allow(dead_code)]

use crate::bmo_core::bef::loader::{self, BinaryFormat, LoadError};
use crate::bmo_core::bef::header::{BefMagic, BEF_MAGIC, BefHeader};
use crate::lang::pipeline::{compile, SourceLang};

pub struct TestResult {
    pub name: &'static str,
    pub passed: bool,
    pub message: alloc::string::String,
}

pub fn run_all() -> alloc::vec::Vec<TestResult> {
    let mut r = alloc::vec::Vec::new();
    r.push(test_magic_detect_bef());
    r.push(test_magic_detect_pe());
    r.push(test_magic_detect_elf());
    r.push(test_magic_detect_unknown());
    r.push(test_load_bef_hello_world());
    r.push(test_load_bef_arithmetic());
    r.push(test_load_bef_invalid_truncated());
    r.push(test_load_bef_invalid_magic());
    r.push(test_load_empty());
    r.push(test_header_from_bytes_valid());
    r.push(test_header_from_bytes_invalid());
    r
}

// ── Magic detection tests ──────────────────────────────────────

fn test_magic_detect_bef() -> TestResult {
    let bytes = b"BEF1\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    match BefMagic::detect(bytes) {
        BefMagic::BefNative => pass("magic_detect_bef", "BEF1 → BefNative"),
        other => fail("magic_detect_bef", &alloc::format!("got {:?}", other)),
    }
}

fn test_magic_detect_pe() -> TestResult {
    let bytes = b"MZ\x90\x00\x03\x00\x00\x00\x04\x00\x00\x00\xff\xff";
    match BefMagic::detect(bytes) {
        BefMagic::PeWindows => pass("magic_detect_pe", "MZ → PeWindows"),
        other => fail("magic_detect_pe", &alloc::format!("got {:?}", other)),
    }
}

fn test_magic_detect_elf() -> TestResult {
    let bytes = b"\x7FELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    match BefMagic::detect(bytes) {
        BefMagic::ElfUnix => pass("magic_detect_elf", "\\x7FELF → ElfUnix"),
        other => fail("magic_detect_elf", &alloc::format!("got {:?}", other)),
    }
}

fn test_magic_detect_unknown() -> TestResult {
    let bytes = b"\xCA\xFE\xBA\xBE";
    match BefMagic::detect(bytes) {
        BefMagic::Unknown => pass("magic_detect_unknown", "cafebabe → Unknown"),
        other => fail("magic_detect_unknown", &alloc::format!("got {:?}", other)),
    }
}

// ── Real BEF load tests ────────────────────────────────────────

fn test_load_bef_hello_world() -> TestResult {
    let src = "\
fn main() {
    diag_print(\"Hello\" as *const u8, 5);
    proc_exit(0);
}
";
    let bytes = match compile(src, SourceLang::Bmo) {
        Ok(b) => b,
        Err(e) => return fail("load_bef_hello_world", &alloc::format!("compile: {:?}", e)),
    };
    match loader::load(&bytes) {
        Ok(img) => {
            if img.format == BinaryFormat::BefNative && img.entry_point > 0 {
                pass("load_bef_hello_world",
                     &alloc::format!("BEF loaded, entry=0x{:x}, sections={}",
                                     img.entry_point, img.sections.len()))
            } else {
                fail("load_bef_hello_world",
                     &alloc::format!("format={:?} entry=0x{:x}", img.format, img.entry_point))
            }
        }
        Err(e) => fail("load_bef_hello_world", &alloc::format!("load: {:?}", e)),
    }
}

fn test_load_bef_arithmetic() -> TestResult {
    let src = "\
fn main() -> num {
    let x: num = 10;
    let y: num = 20;
    let z: num = x + y;
    proc_exit(z);
}
";
    let bytes = match compile(src, SourceLang::Bmo) {
        Ok(b) => b,
        Err(e) => return fail("load_bef_arithmetic", &alloc::format!("compile: {:?}", e)),
    };
    match loader::load(&bytes) {
        Ok(img) => pass("load_bef_arithmetic",
                        &alloc::format!("sections={} entry=0x{:x}",
                                        img.sections.len(), img.entry_point)),
        Err(e) => fail("load_bef_arithmetic", &alloc::format!("load: {:?}", e)),
    }
}

fn test_load_bef_invalid_truncated() -> TestResult {
    let bytes = [0u8; 8]; // Muy corto.
    match loader::load(&bytes) {
        Err(LoadError::UnknownFormat)
        | Err(LoadError::Truncated)
        | Err(LoadError::InvalidHeader) => pass("load_bef_invalid_truncated", "8 bytes rejected"),
        Err(e) => fail("load_bef_invalid_truncated", &alloc::format!("got {:?}", e)),
        Ok(_) => fail("load_bef_invalid_truncated", "8 bytes accepted?"),
    }
}

fn test_load_bef_invalid_magic() -> TestResult {
    let mut bytes = [0u8; 64];
    bytes[0..4].copy_from_slice(b"XXXX"); // Magic inválido.
    match loader::load(&bytes) {
        Err(LoadError::UnknownFormat) => pass("load_bef_invalid_magic", "XXXX → UnknownFormat"),
        Err(e) => fail("load_bef_invalid_magic", &alloc::format!("got {:?}", e)),
        Ok(_) => fail("load_bef_invalid_magic", "invalid magic accepted?"),
    }
}

fn test_load_empty() -> TestResult {
    match loader::load(&[]) {
        Err(_) => pass("load_empty", "0 bytes rejected"),
        Ok(_) => fail("load_empty", "0 bytes accepted?"),
    }
}

// ── Header parsing tests ──────────────────────────────────────

fn test_header_from_bytes_valid() -> TestResult {
    let mut bytes = [0u8; BefHeader::SIZE];
    bytes[0..4].copy_from_slice(&BEF_MAGIC.to_le_bytes());
    bytes[4..6].copy_from_slice(&1u16.to_le_bytes());  // version major
    bytes[6..8].copy_from_slice(&0u16.to_le_bytes());  // version minor
    bytes[12..16].copy_from_slice(&2u32.to_le_bytes()); // section count

    // Safety: BefHeader es repr(C, align(16)), se puede transmutar safely.
    let header: &BefHeader = unsafe { &*(bytes.as_ptr() as *const BefHeader) };
    if header.is_valid() && header.section_count == 2 {
        pass("header_from_bytes_valid", "valid header parsed")
    } else {
        fail("header_from_bytes_valid", "header not valid")
    }
}

fn test_header_from_bytes_invalid() -> TestResult {
    let bytes = [0u8; BefHeader::SIZE];
    let header: &BefHeader = unsafe { &*(bytes.as_ptr() as *const BefHeader) };
    if !header.is_valid() {
        pass("header_from_bytes_invalid", "zeroed header rejected")
    } else {
        fail("header_from_bytes_invalid", "zeroed header accepted?")
    }
}

// ── Helpers ────────────────────────────────────────────────────

fn pass(name: &'static str, msg: &str) -> TestResult {
    TestResult { name, passed: true, message: alloc::string::String::from(msg) }
}
fn fail(name: &'static str, msg: &str) -> TestResult {
    TestResult { name, passed: false, message: alloc::string::String::from(msg) }
}
