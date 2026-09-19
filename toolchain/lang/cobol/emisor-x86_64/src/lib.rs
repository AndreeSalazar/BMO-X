//! **BMO COBOL para x86-64** -- del arbol de COBOL a un `.bex`.
//!
//! [isa] x86-64 -- el UNICO sitio de COBOL que nombra una maquina: el codegen
//! y la edicion emitida. El frontend
//! (`bmo-cobol-front`, la carpeta de arriba) analiza y no sabe de CPU. Partido
//! el 2026-09-18 (`toolchain/tools/isa`).
//!
//! ** Aqui vivia tambien `ir_emit.rs` (COBOL -> la IR de `bmo-abi`). Borrado el
//! 2026-09-18: nadie lo llamaba, y un `DISPLAY` literal salia SIN su cadena
//! (`args: 0`). Cablear o borrar: el codegen de verdad es `codegen.rs`.
//!
//! Todo lo del frontend se re-exporta tal cual (`pub use bmo_cobol_front::*`),
//! asi que `crate::ast`, `crate::registro` o `crate::edicion` siguen siendo los
//! mismos caminos para el codegen y las pruebas.

pub use bmo_cobol_front::*;

pub mod codegen;
pub mod edicion_x86;
mod redondeo;

use bmo_abi::profile::BmoLanguageProfile;

pub fn profile() -> BmoLanguageProfile {
    BmoLanguageProfile::COBOL
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
    let program = parse(source)?;
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
    Ok(d.ver(&elegido, datos, max, &decodificar))
}

/// * El COPYBOOK de un programa: el byte exacto de cada campo de cada registro.
///
/// Sale del PARSER y no del binario a proposito: quien tiene que acordar el
/// formato de un fichero con otro equipo no puede esperar a que el batch este
/// terminado. Y sale de **la misma tabla que usa el codegen** para emitir el
/// `READ` y el `WRITE`, asi que no hay dos sitios donde pueda divergir.
pub fn copybook_de(source: &str) -> Result<String, CobolError> {
    let program = parse(source)?;
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

/// Los decodificadores del visor: los de `bmo-lower`, al lado de sus gemelos
/// emitidos (hay pruebas que los comparan sobre todos los patrones de dos bytes).
fn decodificar(c: registro::Codificacion, trozo: &[u8]) -> Option<i64> {
    match c {
        registro::Codificacion::Empaquetado => Some(bmo_lower::packed::desempaquetar_en_rust(trozo)),
        registro::Codificacion::Zonado => Some(bmo_lower::zoned::leer_en_rust(trozo)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;

/// La prueba de punta a punta del parser por TOKENS: vivia en `tparser.rs`, y
/// su ultima mitad emite un BEF -- asi que es de aqui.
#[cfg(test)]
mod tparser_de_punta_a_punta {
    use crate::tparser::*;
    use crate::ast::*;

    #[test]
    fn parses_whole_program_end_to_end() {
        let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. BANCO.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SALDO PIC 9(5)V99 VALUE 0.
PROCEDURE DIVISION.
MOVE 10.05 TO SALDO.
ADD 3.20 TO SALDO.
DISPLAY \"listo\".
STOP RUN.
";
        let prog = parse_program(src).unwrap();
        assert_eq!(prog.program_id, "BANCO");
        assert_eq!(prog.data_items.len(), 1);
        assert_eq!(prog.data_items[0].name, "SALDO");
        assert_eq!(prog.data_items[0].scale(), 2); // centavos
        assert_eq!(prog.statements.len(), 4);
        assert_eq!(prog.statements[0], CobolStatement::Move("10.05".into(), "SALDO".into()));

        // Pipeline NUEVO completo: tokens -> AST -> BEF (ejecutable real).
        let bef = crate::codegen::compile_to_bef_bytes(&prog).unwrap();
        assert!(bef.len() > 48, "el BEF debe tener cabecera + codigo");
        assert_eq!(&bef[..4], b"BEF1"); // magic del contenedor
    }
}
