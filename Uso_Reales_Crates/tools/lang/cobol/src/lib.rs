pub mod ast;
pub mod codegen;
pub mod ir_emit;
pub mod parser;

use std::path::PathBuf;
use bmo_abi::profile::BmoLanguageProfile;

pub use ast::{CobolCondition, CobolProgram, CobolStatement, DataItem};
pub use ast::error::CobolError;

pub fn profile() -> BmoLanguageProfile {
    BmoLanguageProfile::COBOL
}

pub fn parse(source: &str) -> Result<CobolProgram, CobolError> {
    let mut p = parser::Parser::new(source);
    p.parse_program()
}

pub fn compile_source_to_bef(source: &str) -> Result<Vec<u8>, CobolError> {
    let program = parse(source)?;
    let bytes = codegen::compile_to_bef_bytes(&program)?;
    validate_generated_bef(bytes)
}

pub fn compile_to_ir(source: &str) -> Result<bmo_abi::ir::IrModule, CobolError> {
    let program = parse(source)?;
    Ok(ir_emit::compile_to_ir(&program))
}

pub fn compile_source_to_bef_with_asm(
    source: &str,
    asm_paths: Vec<PathBuf>,
) -> Result<Vec<u8>, CobolError> {
    let mut p = parser::Parser::new(source);
    let program = p.parse_program_with_asm(asm_paths)?;
    let bytes = codegen::compile_to_bef_bytes(&program)?;
    validate_generated_bef(bytes)
}

fn validate_generated_bef(bytes: Vec<u8>) -> Result<Vec<u8>, CobolError> {
    let validation = bmo_abi::bef::validate(&bytes);
    if validation.is_valid {
        return Ok(bytes);
    }

    let details = validation.issues.iter()
        .filter(|issue| matches!(issue.severity, bmo_abi::bef::validator::IssueSeverity::Error))
        .map(|issue| issue.message.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    Err(CobolError::new(0, format!("generated invalid BEF: {details}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parses_display_program() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO-BMO.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-NAME PIC X(20).
PROCEDURE DIVISION.
DISPLAY "HOLA COBOL".
STOP RUN.
"#;
        let program = parse(src).unwrap();
        assert_eq!(program.program_id, "HELLO-BMO");
        assert_eq!(program.data_items.len(), 1);
        assert_eq!(program.data_items[0].name, "WS-NAME");
    }

    #[test]
    fn emits_bef() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO.
PROCEDURE DIVISION.
DISPLAY "HOLA COBOL".
STOP RUN.
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
        assert_eq!(u32::from_le_bytes(bef[..4].try_into().unwrap()), bmo_abi::bef::BEF_MAGIC);
        let validation = bmo_abi::bef::validate(&bef);
        assert!(validation.is_valid, "generated BEF must validate: {:?}", validation.issues);
        let loaded = bmo_abi::bef::load(&bef, 0, bmo_abi::bef::loader::no_imports).unwrap();
        assert_ne!(loaded.entry_point, 0);
        assert!(loaded.sections.iter().any(|section| section.kind == bmo_abi::bef::SectionKind::Code));
    }

    #[test]
    fn parses_arithmetic() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. ARITH.
PROCEDURE DIVISION.
MOVE 10 TO WS-NUM.
ADD 5 TO WS-NUM.
SUBTRACT 3 FROM WS-NUM.
MULTIPLY 2 BY WS-NUM.
DIVIDE 4 BY WS-NUM.
COMPUTE WS-NUM = 10 + 20.
STOP RUN.
"#;
        let program = parse(src).unwrap();
        assert!(program.statements.len() >= 6);
    }

    #[test]
    fn parses_open_read_write_close() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. FILEIO.
PROCEDURE DIVISION.
OPEN INPUT INFILE.
READ INFILE INTO WS-REC.
WRITE OUTFILE.
CLOSE INFILE.
STOP RUN.
"#;
        let program = parse(src).unwrap();
        assert_eq!(program.statements.len(), 5);
    }

    #[test]
    fn parses_perform() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. LOOP.
PROCEDURE DIVISION.
PERFORM 5.
PERFORM UNTIL WS-COUNT > 10.
STOP RUN.
"#;
        let program = parse(src).unwrap();
        assert!(program.statements.len() >= 2);
    }

    #[test]
    fn parses_cobol_use_and_syscall() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TEST.
USE "bmo/proc".
PROCEDURE DIVISION.
SYSCALL bmo_exit 0.
"#;
        let asm = PathBuf::from("X:\\FastOS\\crates_Personal\\Semantic_ASM");
        let p = compile_source_to_bef_with_asm(src, vec![asm]).unwrap();
        assert!(p.len() > 48);
    }

    #[test]
    fn cobol_syscall_with_asm_path() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TEST.
USE "bmo/proc".
PROCEDURE DIVISION.
SYSCALL bmo_exit 42.
"#;
        let asm = PathBuf::from("X:\\FastOS\\crates_Personal\\Semantic_ASM");
        let bef = compile_source_to_bef_with_asm(src, vec![asm]).unwrap();
        assert!(bef.len() > 48);
        let nr = bmo_abi::syscalls::NR_PROC_EXIT;
        let mov_eax = &nr.to_le_bytes();
        let mut expected = vec![0xB8u8];
        expected.extend_from_slice(mov_eax);
        assert!(bef.windows(5).any(|w| w == &expected[..]), "BEF should contain mov eax, NR_PROC_EXIT for bmo_exit");
    }
}
