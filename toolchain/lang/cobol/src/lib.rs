pub mod ast;
pub mod codegen;
pub mod dialect;
pub mod edicion;
pub mod ir_emit;
pub mod lexer;
pub mod parser;
pub mod pic;
/// La disposicion de un registro: que byte ocupa cada campo dentro de su `01`.
/// NO reutiliza el cursor de `bmo-abi` porque aquel ALINEA, y aqui un byte de
/// relleno es un byte que aparece en el disco. Ver la cabecera de `registro.rs`.
pub mod registro;
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
        // Un verbo COBOL reservado pero aun sin codegen -> error que cita el
        // estandar (las tablas generadas por Python alimentan el parser).
        //
        // Era `EVALUATE`, que dejo de servir de ejemplo el 2026-08-03 porque ya
        // compila. Ahora es `CANCEL`, tambien COBOL-85 y tambien sin codegen --
        // y cuando le toque a el, aqui hara falta otro. Que este test haya que
        // cambiarlo es la senal de que el compilador crece.
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nCANCEL X.\n";
        let err = Parser::new(src).parse_program().unwrap_err();
        assert!(err.message.contains("COBOL85"), "esperaba estándar: {}", err.message);
        // Algo que no es COBOL -> error distinto.
        let src2 = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nXYZZY 1.\n";
        let err2 = Parser::new(src2).parse_program().unwrap_err();
        assert!(err2.message.contains("no es COBOL"), "esperaba no-COBOL: {}", err2.message);
    }

    #[test]
    fn standard_tagging_and_intrinsics() {
        use crate::generated::words;
        // Cada palabra sabe de que era viene (Grace Hopper -> ISO 2023).
        assert_eq!(words::reserved_since("MOVE"), Some("COBOL74"));
        assert_eq!(words::reserved_since("EVALUATE"), Some("COBOL85"));
        assert_eq!(words::reserved_since("CLASS-ID"), Some("COBOL2002"));
        assert_eq!(words::reserved_since("JSON"), Some("COBOL2023"));
        assert_eq!(words::reserved_since("NOPE"), None);
        // Funciones intrinsecas.
        assert!(words::is_intrinsic("CURRENT-DATE"));
        assert!(words::is_intrinsic("NUMVAL"));
        assert!(!words::is_intrinsic("MOVE"));
    }

    #[test]
    fn essence_vs_vendor_separation() {
        use crate::generated::words;
        // Esencia estandar (Grace Hopper -> ISO): el nucleo del idioma.
        assert!(words::is_essence("MOVE"));
        assert!(words::is_essence("PERFORM"));
        assert!(words::is_essence("OCCURS"));
        assert!(!words::is_vendor("MOVE"));
        // Extensiones de vendor (VAX DBMS / IBM obsoletas): reconocidas pero
        // NO son la esencia -- BMO COBOL las devora pero las marca aparte.
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
/// ABI v2 surface -- only what the parser accepts changes.
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

/// * EL VISOR: un fichero de registros binarios, decodificado con el copybook
/// del programa que lo escribio.
///
/// `registro` elige cual de los `01` se usa; si es `None`, se coge el primero
/// que cuelgue de un `FD` -- que es el que de verdad cruza al disco.
///
/// Lee con **la misma regla** que escribio el programa: los decodificadores son
/// los de `bmo-lower`, y hay tests que los comparan contra los EMITIDOS sobre
/// todos los patrones de dos bytes.
pub fn ver_registros(
    source: &str,
    datos: &[u8],
    registro: Option<&str>,
    max: usize,
) -> Result<String, CobolError> {
    let program = parser::Parser::new(source).parse_program()?;
    let d = registro::calcular(&program.data_items)?;
    let elegido = match registro {
        Some(r) => r.to_string(),
        None => program
            .files
            .iter()
            .map(|f| f.record.clone())
            .find(|r| !r.is_empty())
            .ok_or_else(|| {
                CobolError::new(
                    0,
                    "este programa no tiene ningun FD con registro: di cual mirar con \
                     `--registro <nombre>`",
                )
            })?,
    };
    Ok(d.ver(&elegido, datos, max))
}

/// * El COPYBOOK de un programa: el byte exacto de cada campo de cada registro.
///
/// Sale del PARSER y no del binario a proposito: quien tiene que acordar el
/// formato de un fichero con otro equipo no puede esperar a que el batch este
/// terminado. Y sale de **la misma tabla que usa el codegen** para emitir el
/// `READ` y el `WRITE`, asi que no hay dos sitios donde pueda divergir.
pub fn copybook_de(source: &str) -> Result<String, CobolError> {
    let program = parser::Parser::new(source).parse_program()?;
    let d = registro::calcular(&program.data_items)?;
    let registros: Vec<String> = program.files.iter().map(|f| f.record.clone()).collect();
    Ok(d.copybook(&program.program_id, &registros))
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
mod tests;
