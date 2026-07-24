pub mod ast;
pub mod codegen;
pub mod dialect;
pub mod ir_emit;
pub mod lexer;
pub mod parser;
pub mod pic;
pub mod tparser;
/// Tablas de COBOL GENERADAS por `toolchain/tools/cobol-gen` (Python).
/// Crecer `definition.py` y regenerar hace crecer esto solo.
pub mod generated {
    pub mod words;
}

#[cfg(test)]
mod generated_tests {
    #[test]
    fn reserved_words_and_verbs() {
        use crate::generated::words;
        assert!(words::is_reserved("DISPLAY"));
        assert!(words::is_reserved("PICTURE"));
        assert!(words::is_reserved("EVALUATE")); // COBOL-85
        assert!(words::is_reserved("INVOKE"));   // COBOL-2002 (OO)
        assert!(words::is_reserved("JSON"));     // COBOL-2023
        assert!(!words::is_reserved("HELLO"));
        assert_eq!(words::verb_kind("MOVE"), Some("Move"));
        assert_eq!(words::verb_kind("STOP"), Some("StopRun"));
        assert_eq!(words::verb_kind("NOTAVERB"), None);
    }

    #[test]
    fn parser_knows_full_cobol_vocabulary() {
        use crate::parser::Parser;
        // Un verbo COBOL reservado pero aún sin codegen → error que cita el
        // estándar (las tablas generadas por Python alimentan el parser).
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nEVALUATE X.\n";
        let err = Parser::new(src).parse_program().unwrap_err();
        assert!(err.message.contains("COBOL85"), "esperaba estándar: {}", err.message);
        // Algo que no es COBOL → error distinto.
        let src2 = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nXYZZY 1.\n";
        let err2 = Parser::new(src2).parse_program().unwrap_err();
        assert!(err2.message.contains("no es COBOL"), "esperaba no-COBOL: {}", err2.message);
    }

    #[test]
    fn standard_tagging_and_intrinsics() {
        use crate::generated::words;
        // Cada palabra sabe de qué era viene (Grace Hopper -> ISO 2023).
        assert_eq!(words::reserved_since("MOVE"), Some("COBOL74"));
        assert_eq!(words::reserved_since("EVALUATE"), Some("COBOL85"));
        assert_eq!(words::reserved_since("CLASS-ID"), Some("COBOL2002"));
        assert_eq!(words::reserved_since("JSON"), Some("COBOL2023"));
        assert_eq!(words::reserved_since("NOPE"), None);
        // Funciones intrínsecas.
        assert!(words::is_intrinsic("CURRENT-DATE"));
        assert!(words::is_intrinsic("NUMVAL"));
        assert!(!words::is_intrinsic("MOVE"));
    }

    #[test]
    fn essence_vs_vendor_separation() {
        use crate::generated::words;
        // Esencia estándar (Grace Hopper → ISO): el núcleo del idioma.
        assert!(words::is_essence("MOVE"));
        assert!(words::is_essence("PERFORM"));
        assert!(words::is_essence("OCCURS"));
        assert!(!words::is_vendor("MOVE"));
        // Extensiones de vendor (VAX DBMS / IBM obsoletas): reconocidas pero
        // NO son la esencia — BMO COBOL las devora pero las marca aparte.
        assert!(words::is_vendor("CONNECT"));   // VAX DBMS
        assert!(words::is_vendor("EXAMINE"));   // IBM obsoleta
        assert!(!words::is_essence("CONNECT"));
    }
}

use std::path::PathBuf;
use bmo_abi::profile::BmoLanguageProfile;

pub use ast::{CobolCondition, CobolProgram, CobolStatement, DataItem};
pub use ast::error::CobolError;

pub fn profile() -> BmoLanguageProfile {
    BmoLanguageProfile::COBOL
}

pub use dialect::{Dialect, DialectConfig, SourceFormat};

pub fn parse(source: &str) -> Result<CobolProgram, CobolError> {
    parse_with_dialect(source, DialectConfig::default())
}

/// Parse under an explicit dialect. Every dialect lowers to the same BMO
/// ABI v2 surface — only what the parser accepts changes.
pub fn parse_with_dialect(
    source: &str,
    dialect: DialectConfig,
) -> Result<CobolProgram, CobolError> {
    let _ = dialect; // v1: the parser accepts the permissive union; the
                     // config gates dialect-specific syntax as it lands.
    let mut p = parser::Parser::new(source);
    p.parse_program()
}

pub fn compile_source_to_bef(source: &str) -> Result<Vec<u8>, CobolError> {
    compile_source_to_bex(source)
}

/// Compile COBOL source into a native BMO executable image.
///
/// BEX v1 uses the validated BEF1 wire format defined by `bmo-abi`.
pub fn compile_source_to_bex(source: &str) -> Result<Vec<u8>, CobolError> {
    let program = parse(source)?;
    let bytes = codegen::compile_to_bef_bytes(&program)?;
    validate_generated_bex(bytes)
}

pub fn compile_to_ir(source: &str) -> Result<bmo_abi::ir::IrModule, CobolError> {
    let program = parse(source)?;
    Ok(ir_emit::compile_to_ir(&program))
}

pub fn compile_source_to_bef_with_asm(
    source: &str,
    asm_paths: Vec<PathBuf>,
) -> Result<Vec<u8>, CobolError> {
    compile_source_to_bex_with_asm(source, asm_paths)
}

/// Compile COBOL source into BEX while using extra semantic-assembly paths.
pub fn compile_source_to_bex_with_asm(
    source: &str,
    asm_paths: Vec<PathBuf>,
) -> Result<Vec<u8>, CobolError> {
    let mut p = parser::Parser::new(source);
    let program = p.parse_program_with_asm(asm_paths)?;
    let bytes = codegen::compile_to_bef_bytes(&program)?;
    validate_generated_bex(bytes)
}

fn validate_generated_bex(bytes: Vec<u8>) -> Result<Vec<u8>, CobolError> {
    let validation = bmo_abi::bex::validate(&bytes);
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

    /// El puente L2→L1: `DISPLAY "texto"` debe bajar a la puerta de consola
    /// del ABI, con el salto de línea que COBOL exige al final (cada DISPLAY
    /// ocupa su propia fila porque `\n` dispara el flush del kernel).
    ///
    /// Antes de esto, COBOL emitía `syscall NR_DEBUG_PRINT` con un puntero —
    /// número que el kernel no despacha y forma que la superficie congelada
    /// rechaza. En hardware no imprimía nada.
    #[test]
    fn display_lowers_to_the_console_door() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO.
PROCEDURE DIVISION.
DISPLAY "HOLA COBOL".
STOP RUN.
"#;
        let bef = compile_source_to_bef(src).unwrap();
        let mut door = Vec::new();
        bmo_lower::console::write_const(&mut door, b"HOLA COBOL\n");
        assert!(
            !door.is_empty() && bef.windows(door.len()).any(|w| w == door),
            "el BEF debe contener la secuencia INVOKE/CONSOLE_WRITE de la puerta"
        );
    }

    /// El cierre del programa no puede usar `hlt`: es privilegiada, y en
    /// Ring 3 provoca el #GP del que pretendía proteger.
    #[test]
    fn program_epilogue_has_no_privileged_instruction() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO.
PROCEDURE DIVISION.
DISPLAY "X".
STOP RUN.
"#;
        let bef = compile_source_to_bef(src).unwrap();
        let mut net = Vec::new();
        bmo_lower::task::exit(&mut net);
        assert!(
            bef.windows(net.len()).any(|w| w == net),
            "el epílogo debe ser INVOKE(EXIT) + red de pause/jmp"
        );
    }

    #[test]
    fn emits_valid_bex_image() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO-BEX.
PROCEDURE DIVISION.
DISPLAY "HOLA BMO".
STOP RUN.
"#;
        let bex = compile_source_to_bex(src).unwrap();
        assert!(bmo_abi::bex::validate(&bex).is_valid);
        assert_eq!(bmo_abi::bex::BEX_WIRE_MAGIC, bmo_abi::bef::BEF_MAGIC);
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
        let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
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
        let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
        let bef = compile_source_to_bef_with_asm(src, vec![asm]).unwrap();
        assert!(bef.len() > 48);
        let nr = bmo_abi::syscalls::surface::NR_INVOKE;
        let mov_eax = &nr.to_le_bytes();
        let mut expected = vec![0xB8u8];
        expected.extend_from_slice(mov_eax);
        assert!(bef.windows(5).any(|w| w == &expected[..]), "BEF should lower bmo_exit to BMO_INVOKE");

        let mut current_task = vec![0x48, 0xB8];
        current_task.extend_from_slice(&bmo_abi::syscalls::surface::CURRENT_TASK.to_le_bytes());
        current_task.extend_from_slice(&[0x48, 0x89, 0xC7]); // mov rdi, rax
        assert!(bef.windows(current_task.len()).any(|w| w == &current_task[..]));

        let mut exit_operation = vec![0x48, 0xB8];
        exit_operation.extend_from_slice(&bmo_abi::syscalls::surface::task_op::EXIT.to_le_bytes());
        exit_operation.extend_from_slice(&[0x48, 0x89, 0xC6]); // mov rsi, rax
        assert!(bef.windows(exit_operation.len()).any(|w| w == &exit_operation[..]));

        let mut exit_code = vec![0x48, 0xB8];
        exit_code.extend_from_slice(&42_u64.to_le_bytes());
        exit_code.extend_from_slice(&[0x48, 0x89, 0xC2]); // mov rdx, rax
        assert!(bef.windows(exit_code.len()).any(|w| w == &exit_code[..]));
    }

    #[test]
    fn cobol_syscall_uses_r10_for_fourth_argument() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SYSCALL-ABI.
PROCEDURE DIVISION.
SYSCALL bmo_wm_create_window 17, 34, 51, 68.
"#;
        let bef = compile_source_to_bef(src).unwrap();
        let mut expected = vec![0x48, 0xB8]; // mov rax, 68
        expected.extend_from_slice(&68_u64.to_le_bytes());
        expected.extend_from_slice(&[0x49, 0x89, 0xC2]); // mov r10, rax
        assert!(bef.windows(expected.len()).any(|window| window == expected));
    }
}
