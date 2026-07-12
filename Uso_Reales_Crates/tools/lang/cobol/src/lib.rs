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
    codegen::compile_to_bef_bytes(&program)
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
    codegen::compile_to_bef_bytes(&program)
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
        let mov_eax = &[0xB8u8, 0x81, 0x01, 0x00, 0x00];
        assert!(bef.windows(5).any(|w| w == mov_eax), "BEF should contain mov eax, 0x181 for bmo_exit");
    }
}
