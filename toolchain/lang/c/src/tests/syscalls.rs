//! SYSCALLS: la puerta declarada en TOML
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

#[test]
fn parses_syscall_direct() {
    // Test that a syscall (bmo_exit) is recognized when definitions are loaded
    let src = r#"use "bmo/proc"; int main() { bmo_exit(0); }"#;
    let p = parse(src).unwrap();
    // Without asm_path, bmo_exit is treated as a normal function call
    assert_eq!(p.functions.len(), 1);
}

#[test]
fn parses_syscall_with_asm_defs() {
    use std::path::PathBuf;
    let src = r#"use "bmo/proc"; int main() { bmo_exit(42); }"#;
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../base");
    let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
    let bef = compile_source_to_bef_with_all(src, vec![base], vec![asm]).unwrap();
    assert!(bef.len() > 48);
}

#[test]
fn syscall_arg_count_validation() {
    use std::path::PathBuf;
    // bmo_exit expects 1 arg -> passing 0 should fail
    let src = r#"use "bmo/proc"; int main() { bmo_exit(); }"#;
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../base");
    let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
    let result = compile_source_to_bef_with_all(src, vec![base], vec![asm]);
    assert!(result.is_err(), "should reject wrong arg count");
    if let Err(e) = result {
        assert!(e.message.contains("expects 1"), "error should mention expected arg count: {e:?}");
    }
}

#[test]
fn syscall_multiple_categories() {
    use std::path::PathBuf;
    let src = r#"use "bmo/proc"; use "bmo/diag"; int main() { bmo_exit(0); bmo_debug_print("test", 4); }"#;
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../base");
    let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
    let bef = compile_source_to_bef_with_all(src, vec![base], vec![asm]).unwrap();
    assert!(bef.len() > 48);
}

#[test]
fn syscall_all_toml_files_loadable() {
    use std::path::PathBuf;
    // Use every category to verify all .toml files load without error
    let src = r#"
use "bmo/proc";
use "bmo/fs";
use "bmo/mem";
use "bmo/input";
use "bmo/time";
use "bmo/diag";
use "bmo/wm";
use "bmo/draw";
use "bmo/winpaint";
use "bmo/compositor";
use "bmo/audio";
use "bmo/ipc";
use "bmo/surface";
int main() { bmo_exit(0); }
"#;
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../base");
    let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
    let bef = compile_source_to_bef_with_all(src, vec![base], vec![asm]).unwrap();
    assert!(bef.len() > 48);
}

#[test]
fn syscall_emits_correct_code() {
    use std::path::PathBuf;
    let src = r#"use "bmo/proc"; int main() { bmo_exit(42); }"#;
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../base");
    let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
    let bef = compile_source_to_bef_with_all(src, vec![base], vec![asm]).unwrap();
    // BEF validation: magic, correct header, code section present
    assert_eq!(u32::from_le_bytes(bef[..4].try_into().unwrap()), bmo_abi::bef::BEF_MAGIC);
    // The emitted code should contain: mov eax, 0x181 (bmo_exit nr)
    let _code_start = 48; // BEF header is 48 bytes
    // Find b5 81 01 00 00 = mov eax, 0x181 (in little-endian)
    let mov_eax = &[0xB8u8, 0x81, 0x01, 0x00, 0x00]; // mov eax, 0x181
    let found = bef.windows(5).any(|w| w == mov_eax);
    assert!(found, "BEF output should contain mov eax, 0x181 for bmo_exit syscall");
    // Should contain syscall instruction (0F 05)
    let syscall = &[0x0F, 0x05];
    let found_syscall = bef.windows(2).any(|w| w == syscall);
    assert!(found_syscall, "BEF output should contain syscall instruction");
}

