//! El CARGADOR: lo que sale de aqui tiene que poder cargarse
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

#[test]
fn emits_bef() {
    let bef = compile_source_to_bef("int main() { printf(\"HOLA C\"); return 0; }").unwrap();
    assert!(bef.len() > 48);
    assert_eq!(u32::from_le_bytes(bef[..4].try_into().unwrap()), bmo_abi::bef::BEF_MAGIC);
}

#[test]
fn emits_bef_with_correct_string_offset() {
    use bmo_abi::bef::sections::{SectionEntry, SectionKind};
    let bef = compile_source_to_bef("int main() { printf(\"HOLA C\"); return 0; }").unwrap();
    let sec_off = u64::from_le_bytes(bef[32..40].try_into().unwrap()) as usize;
    let hdr = unsafe { &*(bef.as_ptr() as *const bmo_abi::bef::header::BefHeader) };
    let count = hdr.section_count as usize;
    // Find rodata section
    let mut rodata_off = 0usize;
    let mut rodata_sz = 0usize;
    for i in 0..count {
        let entry_off = sec_off + i * SectionEntry::SIZE;
        let kind = bef[entry_off];
        if kind == SectionKind::RoData as u8 {
            rodata_off = u64::from_le_bytes(bef[entry_off+8..entry_off+16].try_into().unwrap()) as usize;
            rodata_sz = u64::from_le_bytes(bef[entry_off+16..entry_off+24].try_into().unwrap()) as usize;
            break;
        }
    }
    assert!(rodata_sz > 0, "rodata section not found");
    let rodata = &bef[rodata_off..rodata_off+rodata_sz];
    let end = rodata.iter().position(|&b| b == 0).unwrap();
    let s = core::str::from_utf8(&rodata[..end]).unwrap();
    assert_eq!(s, "HOLA C");
}

#[test]
fn loads_via_bef_loader() {
    use bmo_abi::bef::loader::{load, no_imports};
    use bmo_abi::bef::sections::SectionKind;
    let bef = compile_source_to_bef("int main() { return 42; }").unwrap();
    let loaded = load(&bef, 0, no_imports).unwrap();
    assert!(loaded.entry_point > 0, "entry_point should be non-zero");
    let has_code = loaded.sections.iter().any(|s| s.kind == SectionKind::Code);
    assert!(has_code, "should have Code section");
    // Code section should contain a RET instruction at minimum
    let code = loaded.sections.iter().find(|s| s.kind == SectionKind::Code).unwrap();
    assert!(code.size >= 16, "code section should be at least 16 bytes");
    // Should have non-zero base address
    assert!(loaded.base_addr > 0, "base_addr should be non-zero");
}

#[test]
fn loaded_bef_has_rodata() {
    use bmo_abi::bef::loader::{load, no_imports};
    use bmo_abi::bef::sections::SectionKind;
    let bef = compile_source_to_bef("int main() { printf(\"hello\"); return 0; }").unwrap();
    let loaded = load(&bef, 0, no_imports).unwrap();
    let has_rodata = loaded.sections.iter().any(|s| s.kind == SectionKind::RoData);
    assert!(has_rodata, "printf should create RoData section with the string");
}

#[test]
fn loaded_bef_has_global_data() {
    use bmo_abi::bef::loader::{load, no_imports};
    use bmo_abi::bef::sections::SectionKind;
    let bef = compile_source_to_bef("int g = 42; int main() { return g; }").unwrap();
    let loaded = load(&bef, 0, no_imports).unwrap();
    let has_data = loaded.sections.iter().any(|s| s.kind == SectionKind::Data);
    assert!(has_data, "global vars should create Data section");
}

