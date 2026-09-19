//! **BMO COBOL -- el FRONTEND**: lexer, parser, arbol, `PICTURE`, la
//! disposicion de los registros y la edicion calculada.
//!
//! [isa] NINGUNA. No nombra una maquina y no depende de nada: lo que emite
//! x86-64 vive en `emisor-x86_64/` (crate `bmo-cobol-x86-64`), igual que INTI y
//! Ada. Partido el 2026-09-18 -- en BMO-X todo es x86-64 MENOS los frontends,
//! que son lo unico agnostico. Ver `toolchain/tools/isa`.

pub mod ast;
pub mod dialect;
pub mod edicion;
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

pub use ast::{CobolCondition, CobolProgram, CobolStatement, DataItem};
pub use ast::error::CobolError;

pub use dialect::{Dialect, DialectConfig, SourceFormat};

/// Analiza un programa COBOL. Las puertas del kernel no se escriben en COBOL:
/// las emite el codegen (`DISPLAY`, `ACCEPT`, `STOP RUN`, ficheros).
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

