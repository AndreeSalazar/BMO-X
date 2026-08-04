pub mod ast;
pub mod codegen;
pub mod dialect;
pub mod edicion;
pub mod ir_emit;
pub mod lexer;
pub mod parser;
pub mod pic;
/// La disposición de un registro: qué byte ocupa cada campo dentro de su `01`.
/// NO reutiliza el cursor de `bmo-abi` porque aquél ALINEA, y aquí un byte de
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
        // Un verbo COBOL reservado pero aún sin codegen → error que cita el
        // estándar (las tablas generadas por Python alimentan el parser).
        //
        // Era `EVALUATE`, que dejó de servir de ejemplo el 2026-08-03 porque ya
        // compila. Ahora es `CANCEL`, también COBOL-85 y también sin codegen —
        // y cuando le toque a él, aquí hará falta otro. Que este test haya que
        // cambiarlo es la señal de que el compilador crece.
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nCANCEL X.\n";
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

/// ★ EL VISOR: un fichero de registros binarios, decodificado con el copybook
/// del programa que lo escribió.
///
/// `registro` elige cuál de los `01` se usa; si es `None`, se coge el primero
/// que cuelgue de un `FD` — que es el que de verdad cruza al disco.
///
/// Lee con **la misma regla** que escribió el programa: los decodificadores son
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

/// ★ El COPYBOOK de un programa: el byte exacto de cada campo de cada registro.
///
/// Sale del PARSER y no del binario a propósito: quien tiene que acordar el
/// formato de un fichero con otro equipo no puede esperar a que el batch esté
/// terminado. Y sale de **la misma tabla que usa el codegen** para emitir el
/// `READ` y el `WRITE`, así que no hay dos sitios donde pueda divergir.
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

    // ── Banco de pruebas: EJECUTAR el programa, no mirarlo ──────────────
    //
    // El flujo de control de COBOL estuvo fingiendo durante toda la vida
    // del frontend: `IF` emitía un `jcc` con desplazamiento 0 que nadie
    // parcheaba (o sea, ejecutaba las DOS ramas) y `PERFORM` emitía
    // `xor rax,rax` repetido. Compilaba, validaba el BEF y no hacía nada de
    // lo que decía. Ningún test de bytes lo habría cazado — por eso estos
    // corren el programa en el emulador de `bmo-lower` y comparan lo que el
    // kernel habría pintado.

    /// Extrae la sección CODE del BEF para poder ejecutarla.
    fn code_section(bef: &[u8]) -> Vec<u8> {
        use bmo_abi::bef::sections::{SectionEntry, SectionKind};
        let sec_off = u64::from_le_bytes(bef[32..40].try_into().unwrap()) as usize;
        let hdr = unsafe { &*(bef.as_ptr() as *const bmo_abi::bef::header::BefHeader) };
        for i in 0..hdr.section_count as usize {
            let entry = sec_off + i * SectionEntry::SIZE;
            if bef[entry] == SectionKind::Code as u8 {
                let off = u64::from_le_bytes(bef[entry + 8..entry + 16].try_into().unwrap()) as usize;
                let size = u64::from_le_bytes(bef[entry + 16..entry + 24].try_into().unwrap()) as usize;
                return bef[off..off + size].to_vec();
            }
        }
        panic!("el BEF no tiene seccion CODE");
    }

    /// Compila y ejecuta, devolviendo lo que el kernel habría mostrado.
    fn run_cobol(src: &str) -> String {
        use bmo_lower::emu::{run, Machine};
        let bef = compile_source_to_bef(src).expect("el programa debe compilar");
        let machine = run(Machine::new(code_section(&bef)), 200_000);
        assert!(machine.exited, "el programa debe terminar por INVOKE(EXIT)");
        machine.console
    }

    /// Compila y ejecuta CON DISCO: se siembran los ficheros de entrada y se
    /// devuelve `(consola, maquina)` para poder mirar lo que quedó escrito.
    ///
    /// Sin esto, `OPEN`/`READ`/`WRITE` sólo se distinguirían de un no-op
    /// leyendo el ensamblador — que es exactamente lo que este banco de
    /// pruebas existe para no tener que hacer.
    fn run_cobol_con_disco(
        src: &str,
        entrada: &[(&str, &str)],
    ) -> (String, bmo_lower::emu::Machine) {
        use bmo_lower::emu::{run, Machine};
        let bef = compile_source_to_bef(src).expect("el programa debe compilar");
        let mut m = Machine::new(code_section(&bef));
        for (ruta, datos) in entrada {
            m.poner_archivo(ruta, datos.as_bytes());
        }
        let m = run(m, 2_000_000);
        assert!(m.exited, "el programa debe terminar por INVOKE(EXIT)");
        (m.console.clone(), m)
    }

    /// Igual, pero con el disco NEGÁNDOSE a guardar las rutas que se le digan.
    ///
    /// Es el único ayudante que puede probar el camino del `CLOSE` que falla, y
    /// hace falta porque ese camino **no se puede provocar desde COBOL**: el
    /// programa hace lo mismo en los dos casos y es el disco el que decide. Sin
    /// esto, `emit_close` podía escribir `"00"` a pelo y ninguna prueba lo veía.
    fn run_cobol_sin_poder_guardar(
        src: &str,
        entrada: &[(&str, &str)],
        no_guardables: &[&str],
    ) -> (String, bmo_lower::emu::Machine) {
        use bmo_lower::emu::{run, Machine};
        let bef = compile_source_to_bef(src).expect("el programa debe compilar");
        let mut m = Machine::new(code_section(&bef));
        for (ruta, datos) in entrada {
            m.poner_archivo(ruta, datos.as_bytes());
        }
        for ruta in no_guardables {
            m.fallar_al_guardar(ruta);
        }
        let m = run(m, 2_000_000);
        assert!(m.exited, "el programa debe terminar por INVOKE(EXIT)");
        (m.console.clone(), m)
    }

    /// Igual, pero sembrando BYTES CRUDOS. Hace falta desde que un fichero
    /// puede no ser texto: un registro binario tiene nibbles dentro, y pasarlo
    /// por un `&str` lo destrozaría.
    fn run_cobol_con_disco_bytes(
        src: &str,
        entrada: &[(&str, &[u8])],
    ) -> (String, bmo_lower::emu::Machine) {
        use bmo_lower::emu::{run, Machine};
        let bef = compile_source_to_bef(src).expect("el programa debe compilar");
        let mut m = Machine::new(code_section(&bef));
        for (ruta, datos) in entrada {
            m.poner_archivo(ruta, datos);
        }
        let m = run(m, 2_000_000);
        assert!(m.exited, "el programa debe terminar por INVOKE(EXIT)");
        (m.console.clone(), m)
    }

    /// Un programa con DOS ficheros ya declarados: `ENTRADA` (`d/e.txt`) y
    /// `SALIDA` (`d/s.txt`). Cada caso escribe su propia `FILE SECTION` porque
    /// el PIC del registro es justo lo que cambia de un caso a otro.
    fn programa_con_ficheros(decls: &str, body: &str) -> String {
        format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. TEST.\n\
             ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
             SELECT ENTRADA ASSIGN TO \"d/e.txt\".\n\
             SELECT SALIDA ASSIGN TO \"d/s.txt\".\n\
             DATA DIVISION.\n{decls}\nPROCEDURE DIVISION.\n{body}\nSTOP RUN.\n"
        )
    }

    fn program(data: &str, body: &str) -> String {
        format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. TEST.\nDATA DIVISION.\n\
             WORKING-STORAGE SECTION.\n{data}\nPROCEDURE DIVISION.\n{body}\nSTOP RUN.\n"
        )
    }

    /// Igual, pero **sin** añadir el `STOP RUN` del final.
    ///
    /// Con párrafos, el `STOP RUN` que `program` pega al final ya no cae donde
    /// debe: cae DENTRO del último párrafo, así que el programa termina la
    /// primera vez que alguien hace `PERFORM` de él. Quien escribe párrafos
    /// tiene que decir dónde acaba el cuerpo principal, y por eso este ayudante
    /// no lo decide por él.
    fn programa_con_parrafos(data: &str, body: &str) -> String {
        format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. TEST.\nDATA DIVISION.\n\
             WORKING-STORAGE SECTION.\n{data}\nPROCEDURE DIVISION.\n{body}\n"
        )
    }

    /// La versión con ficheros, también sin `STOP RUN` implícito.
    fn ficheros_con_parrafos(decls: &str, body: &str) -> String {
        format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. TEST.\n\
             ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
             SELECT ENTRADA ASSIGN TO \"d/e.txt\".\n\
             SELECT SALIDA ASSIGN TO \"d/s.txt\".\n\
             DATA DIVISION.\n{decls}\nPROCEDURE DIVISION.\n{body}\n"
        )
    }


    /// Matriz de conformidad de COBOL: ejecuta cada verbo y compara.
    ///
    /// Misma idea que la de C. Antes de existir, `IF` ejecutaba las dos
    /// ramas y `PERFORM` no repetía nada — y el BEF validaba.
    #[test]
    fn cobol_feature_matrix_runs_correctly() {
        let cases: &[(&str, &str, &str, &str)] = &[
            ("MOVE literal", "01 A PIC 9(3).", "MOVE 7 TO A.\nIF A = 7\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("MOVE variable", "01 A PIC 9(3).\n01 B PIC 9(3).", "MOVE 5 TO A.\nMOVE A TO B.\nIF B = 5\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("ADD", "01 A PIC 9(3).", "MOVE 2 TO A.\nADD 3 TO A.\nIF A = 5\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("SUBTRACT", "01 A PIC 9(3).", "MOVE 9 TO A.\nSUBTRACT 4 FROM A.\nIF A = 5\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("MULTIPLY", "01 A PIC 9(3).", "MOVE 3 TO A.\nMULTIPLY 4 BY A.\nIF A = 12\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("DIVIDE", "01 A PIC 9(3).", "MOVE 12 TO A.\nDIVIDE 4 BY A.\nIF A = 3\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("COMPUTE", "01 A PIC 9(3).", "COMPUTE A = 2 + 3 * 4.\nIF A = 14\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("COMPUTE parens", "01 A PIC 9(3).", "COMPUTE A = (2 + 3) * 4.\nIF A = 20\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("COMPUTE vars", "01 A PIC 9(3).\n01 B PIC 9(3).", "MOVE 6 TO A.\nMOVE 7 TO B.\nCOMPUTE A = A * B.\nIF A = 42\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("IF/ELSE", "01 A PIC 9(3).", "MOVE 1 TO A.\nIF A > 5\nDISPLAY \"no\"\nELSE\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("IF anidado", "01 A PIC 9(3).", "MOVE 5 TO A.\nIF A > 1\nIF A < 9\nDISPLAY \"ok\"\nEND-IF\nEND-IF.", "ok\n"),
            ("IF con AND", "01 A PIC 9(3).", "MOVE 5 TO A.\nIF A > 1 AND A < 9\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("PERFORM TIMES", "01 A PIC 9(3).", "PERFORM 2 TIMES\nDISPLAY \"ok\"\nEND-PERFORM.", "ok\nok\n"),
            ("PERFORM UNTIL", "01 I PIC 9(3).", "MOVE 0 TO I.\nPERFORM UNTIL I >= 2\nDISPLAY \"ok\"\nADD 1 TO I\nEND-PERFORM.", "ok\nok\n"),
            ("EVALUATE sujeto", "01 T PIC 9.", "MOVE 2 TO T.\nEVALUATE T\nWHEN 1\nDISPLAY \"no\"\nWHEN 2\nDISPLAY \"ok\"\nEND-EVALUATE.", "ok\n"),
            ("EVALUATE OTHER", "01 T PIC 9.", "MOVE 9 TO T.\nEVALUATE T\nWHEN 1\nDISPLAY \"no\"\nWHEN OTHER\nDISPLAY \"ok\"\nEND-EVALUATE.", "ok\n"),
            ("EVALUATE THRU", "01 T PIC 9.", "MOVE 4 TO T.\nEVALUATE T\nWHEN 2 THRU 5\nDISPLAY \"ok\"\nWHEN OTHER\nDISPLAY \"no\"\nEND-EVALUATE.", "ok\n"),
            ("EVALUATE lista", "01 T PIC 9.", "MOVE 7 TO T.\nEVALUATE T\nWHEN 6, 7\nDISPLAY \"ok\"\nWHEN OTHER\nDISPLAY \"no\"\nEND-EVALUATE.", "ok\n"),
            ("EVALUATE TRUE", "01 S PIC S9(5)V99.", "MOVE 500.00 TO S.\nEVALUATE TRUE\nWHEN S > 1000.00\nDISPLAY \"no\"\nWHEN S > 100.00\nDISPLAY \"ok\"\nWHEN OTHER\nDISPLAY \"no\"\nEND-EVALUATE.", "ok\n"),
            ("OR en IF", "01 A PIC 9(3).", "MOVE 0 TO A.\nIF A = 9 OR A = 0\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("88 con THRU", "01 D PIC 9.\n88 LABORABLE VALUE 1 THRU 5.", "MOVE 3 TO D.\nIF LABORABLE\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("VALUE inicial", "01 A PIC S9(5)V99 VALUE 12.34.", "DISPLAY A.", "12.34\n"),
            ("PERFORM VARYING", "01 I PIC 9(3).\n01 S PIC 9(5) VALUE ZERO.", "PERFORM VARYING I FROM 1 BY 1 UNTIL I > 4\nADD I TO S\nEND-PERFORM.\nDISPLAY S.", "10\n"),
            ("VARYING AFTER", "01 I PIC 9(3).\n01 J PIC 9(3).\n01 N PIC 9(4) VALUE ZERO.", "PERFORM VARYING I FROM 1 BY 1 UNTIL I > 2\nAFTER J FROM 1 BY 1 UNTIL J > 3\nADD 1 TO N\nEND-PERFORM.\nDISPLAY N.", "6\n"),
            ("ROUNDED", "01 A PIC S9(5)V99 VALUE 10.00.", "DIVIDE 7 BY A ROUNDED.\nDISPLAY A.", "1.43\n"),
            ("ON SIZE ERROR", "01 A PIC 9(3) VALUE 999.", "ADD 999 TO A ON SIZE ERROR\nDISPLAY \"no cabe\"\nEND-ADD.\nDISPLAY A.", "no cabe\n999\n"),
            ("PIC X", "01 T PIC X(6) VALUE \"HOLA\".", "DISPLAY T.", "HOLA  \n"),
            ("texto compara", "01 T PIC XX VALUE \"00\".", "IF T = \"00\"\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("INSPECT", "01 T PIC X(7) VALUE \"  12 34\".", "INSPECT T REPLACING LEADING SPACE BY ZERO.\nDISPLAY T.", "0012 34\n"),
            ("STRING", "01 A PIC X(2) VALUE \"AB\".\n01 C PIC X(5).", "STRING A DELIMITED BY SIZE \"-\" DELIMITED BY SIZE A DELIMITED BY SIZE INTO C.\nDISPLAY C.", "AB-AB\n"),
            ("PERFORM anidado", "01 I PIC 9(3).", "PERFORM 2 TIMES\nPERFORM 2 TIMES\nDISPLAY \"ok\"\nEND-PERFORM\nEND-PERFORM.", "ok\nok\nok\nok\n"),
            ("decimal exacto", "01 S PIC 9(5)V99.", "MOVE 10.05 TO S.\nADD 0.20 TO S.\nIF S = 10.25\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("escalas mixtas", "01 S PIC 9(5)V99.\n01 N PIC 9(3).", "MOVE 2 TO N.\nMOVE 1.50 TO S.\nADD N TO S.\nIF S = 3.50\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("cond en palabras", "01 A PIC 9(3).", "MOVE 5 TO A.\nIF A IS EQUAL TO 5\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            // ── PICTURE de edición, EN EJECUCIÓN ──
            //
            // La línea del extracto. El dato sigue siendo un entero de
            // centavos; lo que cambia es que al enseñarlo se recorre la
            // máscara, y ese recorrido son instrucciones dentro del .bex.
            ("PIC moneda", "01 L PIC $$$,$$9.99.", "MOVE 12345.67 TO L.\nDISPLAY L.", "$12,345.67\n"),
            ("PIC moneda pequena", "01 L PIC $$$,$$9.99.", "MOVE 0.45 TO L.\nDISPLAY L.", "     $0.45\n"),
            // ★ El símbolo flotante cuando la supresión muere JUSTO tras la
            // coma: el `$` va en la casilla de la coma, porque los separadores
            // de dentro del grupo flotante son parte del grupo. Daba
            // `  $ 105.00` —con un hueco en medio— y los 238 casos de
            // `edicion.rs` no lo veían porque comparan las dos
            // implementaciones entre sí, y las dos se equivocaban igual.
            ("PIC moneda tras coma", "01 L PIC $$$,$$9.99.", "MOVE 105.00 TO L.\nDISPLAY L.", "   $105.00\n"),
            ("PIC cheque", "01 L PIC **,**9.99.", "MOVE 0.45 TO L.\nDISPLAY L.", "*****0.45\n"),
            ("PIC saldo en rojo", "01 L PIC Z,ZZ9.99CR.", "MOVE -120.00 TO L.\nDISPLAY L.", "  120.00CR\n"),
            ("PIC saldo en verde", "01 L PIC Z,ZZ9.99CR.", "MOVE 120.00 TO L.\nDISPLAY L.", "  120.00  \n"),
            ("PIC supresion", "01 L PIC Z,ZZ9.", "MOVE 7 TO L.\nDISPLAY L.", "    7\n"),
            ("PIC signo flotante", "01 L PIC ---9.", "MOVE -7 TO L.\nDISPLAY L.", "  -7\n"),
            // La edición no toca la aritmética: el campo se totaliza como
            // cualquier otro y sólo al final se enseña con su máscara.
            ("PIC se puede sumar", "01 L PIC $$$,$$9.99.", "MOVE 10.05 TO L.\nADD 0.20 TO L.\nDISPLAY L.", "    $10.25\n"),
            // Y el signo del literal sobrevive al camino entero.
            ("literal negativo", "01 A PIC S9(3)V99.", "MOVE -1.50 TO A.\nIF A < 0\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            // ── NIVEL 88: nombres de condición ──
            //
            // Un 88 no ocupa memoria: le pone nombre a una comparación. Es lo
            // que convierte `PERFORM UNTIL FIN = 1` en
            // `PERFORM UNTIL FIN-DE-FICHERO`, que es COBOL bancario del que se
            // lee en voz alta.
            ("88 verdadero", "01 F PIC 9.\n88 FIN VALUE 1.", "MOVE 1 TO F.\nIF FIN\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("88 falso", "01 F PIC 9.\n88 FIN VALUE 1.", "MOVE 0 TO F.\nIF FIN\nDISPLAY \"no\"\nELSE\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            // ★ Para lo que existe: el bucle del batch, legible.
            ("88 en PERFORM UNTIL", "01 F PIC 9.\n88 SE-ACABO VALUE 1.\n01 I PIC 9(3).", "MOVE 0 TO F.\nMOVE 0 TO I.\nPERFORM UNTIL SE-ACABO\nADD 1 TO I\nIF I >= 3\nMOVE 1 TO F\nEND-IF\nEND-PERFORM.\nIF I = 3\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            // Varios 88 sobre el MISMO dato, que es como se usa de verdad.
            ("88 varios sobre uno", "01 E PIC 9.\n88 ACTIVO VALUE 1.\n88 CERRADO VALUE 2.", "MOVE 2 TO E.\nIF CERRADO\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            // El 88 cuelga del dato de ARRIBA, no del primero del programa.
            ("88 cuelga del de arriba", "01 A PIC 9.\n01 B PIC 9.\n88 B-ES-CINCO VALUE 5.", "MOVE 5 TO A.\nMOVE 5 TO B.\nIF B-ES-CINCO\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            // Con decimales: el valor se escala como cualquier literal.
            ("88 con decimales", "01 S PIC S9(5)V99.\n88 SALDADO VALUE 0.00.", "MOVE 0 TO S.\nIF SALDADO\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("88 y AND", "01 F PIC 9.\n88 FIN VALUE 1.\n01 N PIC 9(3).", "MOVE 1 TO F.\nMOVE 7 TO N.\nIF FIN AND N = 7\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            // ── OCCURS: tablas ──
            //
            // El subindice literal se resuelve al compilar (sin multiplicar y
            // sin comprobar nada en ejecucion); el variable, con su guarda.
            ("OCCURS literal", "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.", "MOVE 5 TO E(1).\nIF E(1) = 5\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            ("OCCURS variable", "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.\n01 I PIC 9(3).", "MOVE 2 TO I.\nMOVE 7 TO E(I).\nIF E(I) = 7\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            // Cada elemento es SUYO. Si el paso estuviera mal, escribir en el
            // segundo se vería en el primero — y un total por concepto saldría
            // sumado en la casilla del vecino.
            ("OCCURS no se pisan", "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.", "MOVE 1 TO E(1).\nMOVE 2 TO E(2).\nMOVE 3 TO E(3).\nIF E(1) = 1 AND E(2) = 2 AND E(3) = 3\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            // ★ Para lo que existe OCCURS: recorrer la tabla y totalizar.
            ("OCCURS totaliza", "01 T.\n05 E PIC S9(7)V99 OCCURS 3 TIMES.\n01 I PIC 9(3).\n01 TOT PIC S9(7)V99.", "MOVE 10.05 TO E(1).\nMOVE 0.20 TO E(2).\nMOVE 1.75 TO E(3).\nMOVE 0 TO TOT.\nMOVE 1 TO I.\nPERFORM UNTIL I > 3\nADD E(I) TO TOT\nADD 1 TO I\nEND-PERFORM.\nIF TOT = 12.00\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            // Un elemento con PIC editada enseña su mascara, como cualquier
            // otro dato: la edicion es de la tabla, no de la casilla.
            ("OCCURS PIC editada", "01 T.\n05 L PIC $$$,$$9.99 OCCURS 2 TIMES.", "MOVE 10.05 TO L(2).\nDISPLAY L(2).", "    $10.05\n"),
            // El subindice puede ser OTRO elemento de tabla. Es lo que prueba
            // que el valor a guardar sobrevive al calculo de la direccion.
            ("OCCURS subindice de tabla", "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.\n01 X.\n05 IDX PIC 9(3) OCCURS 2 TIMES.", "MOVE 3 TO IDX(1).\nMOVE 9 TO E(IDX(1)).\nIF E(3) = 9\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
            // ★ Y el subindice que se sale PARA el programa diciendo cual.
            // Seguir con una direccion inventada escribiria encima del campo de
            // al lado, y el descuadre apareceria semanas despues en otro sitio.
            ("OCCURS fuera de rango", "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.\n01 I PIC 9(3).", "MOVE 4 TO I.\nMOVE 1 TO E(I).\nDISPLAY \"no deberia llegar\".", "SUBINDICE FUERA DE RANGO EN E (1..3)\n"),
            ("OCCURS subindice cero", "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.\n01 I PIC 9(3).", "MOVE 0 TO I.\nMOVE 1 TO E(I).\nDISPLAY \"no deberia llegar\".", "SUBINDICE FUERA DE RANGO EN E (1..3)\n"),
        ];
        let mut broken = Vec::new();
        for (name, data, body, expected) in cases {
            let src = program(data, body);
            let got = std::panic::catch_unwind(|| run_cobol(&src))
                .unwrap_or_else(|_| "<no ejecuta>".into());
            if got != *expected {
                broken.push(format!("  {name:<18} => {got:?}  (esperado {expected:?})"));
            }
        }

        // ── E/S DE FICHEROS ─────────────────────────────────────────────
        //
        // Estos casos necesitan DISCO, así que van con su propio banco: se
        // siembra `d/e.txt`, se ejecuta, y se mira la consola Y lo que quedó
        // en `d/s.txt`. Mirar sólo la consola dejaría pasar un `WRITE` que no
        // escribe, y mirar sólo el fichero dejaría pasar un `AT END` que nunca
        // salta.
        //
        // Campos: nombre, declaraciones, cuerpo, lo sembrado, consola
        // esperada, y el fichero esperado (`None` = no debe existir).
        let discos: &[(&str, &str, &str, &str, &str, Option<&str>)] = &[
            (
                "OPEN/READ/CLOSE",
                "FILE SECTION.\nFD ENTRADA.\n01 R PIC 9(5).",
                "OPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"vacio\"\nNOT AT END DISPLAY R\nEND-READ.\nCLOSE ENTRADA.",
                "42\n",
                "42\n",
                None,
            ),
            (
                "AT END salta",
                "FILE SECTION.\nFD ENTRADA.\n01 R PIC 9(5).",
                "OPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"ok\"\nEND-READ.\nCLOSE ENTRADA.",
                "",
                "ok\n",
                None,
            ),
            // El bucle del batch: leer hasta el final y totalizar. Es LA forma
            // del proceso por lotes, y sin `AT END` no terminaría nunca.
            (
                "PERFORM sobre fichero",
                "FILE SECTION.\nFD ENTRADA.\n01 R PIC S9(7)V99.\nWORKING-STORAGE SECTION.\n01 T PIC S9(7)V99.\n01 F PIC 9.",
                "MOVE 0 TO T.\nMOVE 0 TO F.\nOPEN INPUT ENTRADA.\nPERFORM UNTIL F = 1\nREAD ENTRADA\nAT END MOVE 1 TO F\nNOT AT END ADD R TO T\nEND-READ\nEND-PERFORM.\nCLOSE ENTRADA.\nIF T = 1235.00\nDISPLAY \"ok\"\nEND-IF.",
                "1000.00\n234.56\n0.44\n",
                "ok\n",
                None,
            ),
            // Un registro leído es un decimal EXACTO, no un float: cinco
            // céntimos no pueden convertirse en cincuenta al cruzar el disco.
            (
                "READ decimal exacto",
                "FILE SECTION.\nFD ENTRADA.\n01 R PIC S9(7)V99.",
                "OPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"no\"\nNOT AT END IF R = 0.05\nDISPLAY \"ok\"\nEND-IF\nEND-READ.\nCLOSE ENTRADA.",
                "0.05\n",
                "ok\n",
                None,
            ),
            (
                "READ negativo",
                "FILE SECTION.\nFD ENTRADA.\n01 R PIC S9(7)V99.",
                "OPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"no\"\nNOT AT END IF R < 0\nDISPLAY \"ok\"\nEND-IF\nEND-READ.\nCLOSE ENTRADA.",
                "-100.00\n",
                "ok\n",
                None,
            ),
            // El fichero viene del anfitrión con `\r\n`. Ese `\r` dentro del
            // número lo convertiría en otro.
            (
                "READ con CRLF",
                "FILE SECTION.\nFD ENTRADA.\n01 R PIC 9(5).",
                "OPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"no\"\nNOT AT END IF R = 77\nDISPLAY \"ok\"\nEND-IF\nEND-READ.\nCLOSE ENTRADA.",
                "77\r\n",
                "ok\n",
                None,
            ),
            // El clásico que se come el movimiento de más valor: el último.
            (
                "READ sin salto final",
                "FILE SECTION.\nFD ENTRADA.\n01 R PIC 9(5).",
                "OPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"no\"\nNOT AT END IF R = 9\nDISPLAY \"ok\"\nEND-IF\nEND-READ.\nCLOSE ENTRADA.",
                "9",
                "ok\n",
                None,
            ),
            (
                "OPEN OUTPUT/WRITE",
                "FILE SECTION.\nFD SALIDA.\n01 S PIC S9(7)V99.",
                "MOVE 1135.00 TO S.\nOPEN OUTPUT SALIDA.\nWRITE S.\nCLOSE SALIDA.",
                "",
                "",
                Some("1135.00\n"),
            ),
            // ★ El registro con PIC editada escribe su LINEA, no su número:
            // eso es un informe bancario. Antes de `emitir_en_buffer` esto
            // habría escrito `10.25` callando que había máscara.
            (
                "WRITE PIC editada",
                "FILE SECTION.\nFD SALIDA.\n01 S PIC $$$,$$9.99.",
                "MOVE 10.05 TO S.\nADD 0.20 TO S.\nOPEN OUTPUT SALIDA.\nWRITE S.\nCLOSE SALIDA.",
                "",
                "",
                Some("    $10.25\n"),
            ),
            (
                "WRITE varias lineas",
                "FILE SECTION.\nFD SALIDA.\n01 S PIC 9(3).",
                "OPEN OUTPUT SALIDA.\nMOVE 1 TO S.\nWRITE S.\nMOVE 2 TO S.\nWRITE S.\nCLOSE SALIDA.",
                "",
                "",
                Some("1\n2\n"),
            ),
            // Sin CLOSE no se guarda NADA. No medio fichero: ninguno. Un
            // extracto truncado se parece demasiado a uno completo.
            (
                "sin CLOSE no se guarda",
                "FILE SECTION.\nFD SALIDA.\n01 S PIC 9(3).",
                "MOVE 7 TO S.\nOPEN OUTPUT SALIDA.\nWRITE S.",
                "",
                "",
                None,
            ),
            // Y la vuelta entera: lo escrito se puede volver a leer. Es el
            // contrato entre `WRITE` y `leer_linea` — un registro por línea.
            (
                "lo escrito se relee",
                "FILE SECTION.\nFD ENTRADA.\n01 R PIC 9(5).\nFD SALIDA.\n01 S PIC 9(5).",
                "MOVE 314 TO S.\nOPEN OUTPUT SALIDA.\nWRITE S.\nCLOSE SALIDA.\nOPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"no\"\nNOT AT END IF R = 314\nDISPLAY \"ok\"\nEND-IF\nEND-READ.\nCLOSE ENTRADA.",
                "314\n",
                "ok\n",
                Some("314\n"),
            ),
        ];
        for (name, decls, body, entrada, esperado, fichero) in discos {
            let src = programa_con_ficheros(decls, body);
            let sembrado: Vec<(&str, &str)> =
                if entrada.is_empty() { vec![] } else { vec![("d/e.txt", entrada)] };
            let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (consola, m) = run_cobol_con_disco(&src, &sembrado);
                (consola, m.archivo_texto("d/s.txt"))
            }));
            match got {
                Err(_) => broken.push(format!("  {name:<22} => <no ejecuta>")),
                Ok((consola, en_disco)) => {
                    if consola != *esperado {
                        broken.push(format!(
                            "  {name:<22} => consola {consola:?}  (esperado {esperado:?})"
                        ));
                    }
                    if en_disco.as_deref() != *fichero {
                        broken.push(format!(
                            "  {name:<22} => disco {en_disco:?}  (esperado {fichero:?})"
                        ));
                    }
                }
            }
        }

        let total = cases.len() + discos.len();
        assert!(broken.is_empty(), "\n{}/{} FUNCIONAN. ROTOS:\n{}", total - broken.len(), total, broken.join("\n"));
    }

    // ── EL ÁREA DE REGISTRO: grupos con los campos en su byte ───────────
    //
    // Camino B de PLAN_BANCA §1.0. Un grupo tiene dos representaciones: las
    // ranuras de trabajo (un entero escalado por campo) y el ÁREA (los bytes
    // tal cual irían al disco). `MOVE` de grupo pasa por el área.

    /// ★ LA PRUEBA QUE NO SE PUEDE FINGIR: un `MOVE` de grupo pasa por los
    /// BYTES, así que sobrevive a que los dos grupos tengan **nombres
    /// distintos** en los campos — sólo tienen que coincidir en la forma.
    ///
    /// Un emisor que copiara campo a campo por nombre fallaría aquí, y uno que
    /// copiara por posición de declaración pasaría este test pero fallaría el
    /// de abajo.
    #[test]
    fn un_move_de_grupo_copia_los_bytes_no_los_nombres() {
        let src = program(
            "01 ORIGEN.\n\
             05 O-A PIC 9(4).\n\
             05 O-B PIC S9(5)V99 COMP-3.\n\
             01 DESTINO.\n\
             05 D-X PIC 9(4).\n\
             05 D-Y PIC S9(5)V99 COMP-3.",
            "MOVE 1234 TO O-A.\nMOVE -99.95 TO O-B.\n\
             MOVE ORIGEN TO DESTINO.\n\
             DISPLAY D-X.\nDISPLAY D-Y.",
        );
        assert_eq!(run_cobol(&src), "1234\n-99.95\n");
    }

    /// ★ Y ÉSTA es la que prueba que el área son BYTES DE VERDAD y no un
    /// atajo: los dos grupos tienen la **misma forma en bytes** pero **cortada
    /// distinta**. Origen: un campo de 6 dígitos. Destino: dos de 3.
    ///
    /// Copiar campo a campo no puede dar esto. Sólo sale bien si lo que viaja
    /// son los bytes zonados — `123456` escrito como seis caracteres, y el
    /// destino leyendo `123` y `456` de su sitio.
    ///
    /// Es exactamente lo que un programa de banca hace para reinterpretar un
    /// registro, y la razón por la que el estándar dice que un `MOVE` de grupo
    /// no mira qué hay dentro.
    #[test]
    fn el_area_son_bytes_de_verdad_y_se_puede_recortar_distinto() {
        let src = program(
            "01 ORIGEN.\n\
             05 O-TODO PIC 9(6).\n\
             01 DESTINO.\n\
             05 D-ALTO PIC 9(3).\n\
             05 D-BAJO PIC 9(3).",
            "MOVE 123456 TO O-TODO.\n\
             MOVE ORIGEN TO DESTINO.\n\
             DISPLAY D-ALTO.\nDISPLAY D-BAJO.",
        );
        assert_eq!(
            run_cobol(&src),
            "123\n456\n",
            "el area no son bytes: el MOVE de grupo copio campo a campo"
        );
    }

    /// El signo sobrevive al viaje por el área, que es donde vive
    /// sobrepunzado en el último dígito.
    #[test]
    fn el_signo_sobrevive_al_area() {
        let src = program(
            "01 ORIGEN.\n05 O-A PIC S9(5).\n\
             01 DESTINO.\n05 D-A PIC S9(5).",
            "MOVE -1234 TO O-A.\nMOVE ORIGEN TO DESTINO.\nDISPLAY D-A.",
        );
        assert_eq!(run_cobol(&src), "-1234\n");
    }

    /// Un grupo dentro de otro: los offsets se acumulan y el `MOVE` de arriba
    /// se lleva todo lo de abajo.
    #[test]
    fn un_move_de_grupo_arrastra_los_grupos_de_dentro() {
        let src = program(
            "01 ORIGEN.\n\
             05 O-CAB.\n\
             10 O-TIPO PIC 9.\n\
             10 O-NUM PIC 9(4).\n\
             05 O-IMP PIC S9(5)V99 COMP-3.\n\
             01 DESTINO.\n\
             05 D-CAB.\n\
             10 D-TIPO PIC 9.\n\
             10 D-NUM PIC 9(4).\n\
             05 D-IMP PIC S9(5)V99 COMP-3.",
            "MOVE 7 TO O-TIPO.\nMOVE 4471 TO O-NUM.\nMOVE 1234.56 TO O-IMP.\n\
             MOVE ORIGEN TO DESTINO.\n\
             DISPLAY D-TIPO.\nDISPLAY D-NUM.\nDISPLAY D-IMP.",
        );
        assert_eq!(run_cobol(&src), "7\n4471\n1234.56\n");
    }

    /// Dos campos con el mismo nombre no se pueden distinguir en un `MOVE`.
    /// COBOL lo resuelve con `A OF REG`, que todavía no existe — así que se
    /// dice en vez de quedarse con uno de los dos en silencio.
    #[test]
    fn dos_campos_con_el_mismo_nombre_se_rechazan() {
        let src = program(
            "01 UNO.\n05 IMPORTE PIC 9(4).\n01 DOS.\n05 IMPORTE PIC 9(4).",
            "DISPLAY \"x\".",
        );
        let err = compile_source_to_bef(&src).unwrap_err().to_string();
        assert!(err.contains("dos veces"), "{err}");
    }

    /// Mezclar un grupo con un campo pide relleno con espacios, y eso necesita
    /// que exista el texto. Se dice en vez de mover el primer campo y callar.
    #[test]
    fn mover_un_grupo_a_un_campo_se_rechaza() {
        let src = program(
            "01 G.\n05 A PIC 9(4).\n01 SUELTO PIC 9(4).",
            "MOVE G TO SUELTO.",
        );
        let err = compile_source_to_bef(&src).unwrap_err().to_string();
        assert!(err.contains("uno es un GRUPO"), "{err}");
    }

    // ── PERFORM VARYING: el bucle CON ÍNDICE ────────────────────────────

    /// Lo mínimo, y con el índice usable dentro del cuerpo.
    #[test]
    fn perform_varying_recorre_con_indice() {
        let src = program(
            "01 I PIC 9(3).\n01 SUMA PIC 9(5) VALUE ZERO.",
            "PERFORM VARYING I FROM 1 BY 1 UNTIL I > 5\n\
             ADD I TO SUMA\n\
             END-PERFORM.\n\
             DISPLAY SUMA.\nDISPLAY I.",
        );
        // 1+2+3+4+5 = 15, y al salir I vale 6 — la vuelta que hizo fallar la
        // condición también incrementó.
        assert_eq!(run_cobol(&src), "15\n6\n");
    }

    /// ⚠ `UNTIL` dice cuándo **PARAR**, no cuándo seguir. Es al revés que el
    /// `while` de casi todo lo demás, y confundirlo da una vuelta de más o de
    /// menos — que sobre una tabla es un subíndice fuera de rango.
    #[test]
    fn el_until_dice_cuando_parar() {
        // `UNTIL I > 3` recorre 1,2,3 — no llega al 4.
        let src = program(
            "01 I PIC 9(3).\n01 T PIC X(8) VALUE SPACES.",
            "PERFORM VARYING I FROM 1 BY 1 UNTIL I > 3\n\
             DISPLAY I\n\
             END-PERFORM.",
        );
        assert_eq!(run_cobol(&src), "1\n2\n3\n");
    }

    /// `WITH TEST BEFORE`: si la condición ya se cumple al entrar, el cuerpo
    /// **no corre ni una vez**.
    #[test]
    fn si_ya_se_cumple_no_da_ni_una_vuelta() {
        let src = program(
            "01 I PIC 9(3).",
            "PERFORM VARYING I FROM 9 BY 1 UNTIL I > 3\n\
             DISPLAY \"no deberia\"\n\
             END-PERFORM.\nDISPLAY \"fin\".",
        );
        assert_eq!(run_cobol(&src), "fin\n");
    }

    /// El paso puede ser distinto de uno, y **hacia atrás**.
    #[test]
    fn el_paso_puede_ir_hacia_atras() {
        let src = program(
            "01 I PIC S9(3).",
            "PERFORM VARYING I FROM 10 BY -3 UNTIL I < 1\n\
             DISPLAY I\n\
             END-PERFORM.",
        );
        assert_eq!(run_cobol(&src), "10\n7\n4\n1\n");
    }

    /// ★ `AFTER` — y lo que de verdad prueba: el de dentro **se reinicia** cada
    /// vez que el de fuera avanza.
    ///
    /// Sin ese reinicio la tabla se recorre en diagonal: la primera fila entera
    /// y de las demás sólo la última columna. Por eso el test cuenta las
    /// vueltas: tienen que ser 3 × 4, no 3 + 4.
    #[test]
    fn el_after_se_reinicia_en_cada_vuelta_de_fuera() {
        let src = program(
            "01 I PIC 9(3).\n01 J PIC 9(3).\n01 N PIC 9(4) VALUE ZERO.",
            "PERFORM VARYING I FROM 1 BY 1 UNTIL I > 3\n\
             AFTER J FROM 1 BY 1 UNTIL J > 4\n\
             ADD 1 TO N\n\
             END-PERFORM.\n\
             DISPLAY N.",
        );
        assert_eq!(run_cobol(&src), "12\n", "el AFTER no se reinicio: la tabla se recorrio mal");
    }

    /// Tres niveles, para que no pase por casualidad con dos.
    #[test]
    fn se_pueden_encadenar_tres() {
        let src = program(
            "01 I PIC 9(3).\n01 J PIC 9(3).\n01 K PIC 9(3).\n01 N PIC 9(4) VALUE ZERO.",
            "PERFORM VARYING I FROM 1 BY 1 UNTIL I > 2\n\
             AFTER J FROM 1 BY 1 UNTIL J > 3\n\
             AFTER K FROM 1 BY 1 UNTIL K > 5\n\
             ADD 1 TO N\n\
             END-PERFORM.\n\
             DISPLAY N.",
        );
        assert_eq!(run_cobol(&src), "30\n"); // 2 × 3 × 5
    }

    /// ★ EL CASO POR EL QUE EXISTE: recorrer una tabla con `OCCURS`.
    #[test]
    fn perform_varying_recorre_una_tabla() {
        let src = program(
            "01 TABLA.\n05 T PIC S9(5)V99 OCCURS 4 TIMES.\n\
             01 I PIC 9(3).\n01 TOTAL PIC S9(7)V99 VALUE ZERO.",
            "MOVE 10.01 TO T(1).\nMOVE 20.02 TO T(2).\n\
             MOVE 30.03 TO T(3).\nMOVE 40.04 TO T(4).\n\
             PERFORM VARYING I FROM 1 BY 1 UNTIL I > 4\n\
             ADD T(I) TO TOTAL\n\
             END-PERFORM.\n\
             DISPLAY TOTAL.",
        );
        assert_eq!(run_cobol(&src), "100.10\n");
    }

    /// La forma FUERA DE LÍNEA: el cuerpo es un párrafo.
    #[test]
    fn perform_varying_de_parrafo() {
        let src = programa_con_parrafos(
            "01 I PIC 9(3).\n01 SUMA PIC 9(5) VALUE ZERO.",
            "PERFORM 1000-SUMA VARYING I FROM 1 BY 1 UNTIL I > 4.\n\
             DISPLAY SUMA.\n\
             STOP RUN.\n\
             1000-SUMA.\n\
             ADD I TO SUMA.",
        );
        assert_eq!(run_cobol(&src), "10\n"); // 1+2+3+4
    }

    /// Lo que falta se dice.
    #[test]
    fn los_varying_incompletos_se_rechazan() {
        let casos: &[(&str, &str)] = &[
            ("PERFORM VARYING I FROM 1 UNTIL I > 3\nDISPLAY I\nEND-PERFORM.", "las tres partes"),
            ("PERFORM VARYING I BY 1 FROM 1 UNTIL I > 3\nDISPLAY I\nEND-PERFORM.", "el orden es FROM"),
            ("PERFORM VARYING I FROM 1 BY 1 UNTIL I > 3\nDISPLAY I.", "END-PERFORM"),
        ];
        for (body, pista) in casos {
            let src = program("01 I PIC 9(3).", body);
            let err = compile_source_to_bef(&src)
                .expect_err(&format!("deberia rechazarse: {body}"))
                .to_string();
            assert!(err.contains(pista), "{body}\n => {err:?}");
        }
    }

    // ── GO TO: el descarte dentro de un rango ───────────────────────────

    /// ★ EL CASO POR EL QUE EXISTE: descartar un registro dentro de un
    /// `PERFORM … THRU`, saltando al párrafo de salida.
    ///
    /// Es lo que el ejemplo del nivel 8 escribía con un interruptor porque esto
    /// no existía — y lo decía ahí mismo, porque fingirlo con un `PERFORM` del
    /// párrafo de salida no vale: aquél lo ejecuta y **vuelve**, así que el
    /// trabajo de debajo se hace igual.
    #[test]
    fn go_to_descarta_dentro_de_un_rango() {
        let src = programa_con_parrafos(
            "01 I PIC 9(3) VALUE ZERO.\n01 CONTADOS PIC 9(3) VALUE ZERO.",
            "PERFORM 1000-VALIDA THRU 1000-SALIR.\n\
             MOVE 5 TO I.\n\
             PERFORM 1000-VALIDA THRU 1000-SALIR.\n\
             DISPLAY CONTADOS.\n\
             STOP RUN.\n\
             1000-VALIDA.\n\
             IF I = 0\n\
             GO TO 1000-SALIR\n\
             END-IF.\n\
             1100-CUENTA.\n\
             ADD 1 TO CONTADOS.\n\
             1000-SALIR.\n\
             EXIT.",
        );
        // La primera vuelta descarta (I = 0), la segunda cuenta.
        assert_eq!(run_cobol(&src), "1\n", "el GO TO no salto el trabajo de en medio");
    }

    /// Y **vuelve al PERFORM que lo llamó**: después del rango, el cuerpo
    /// principal sigue. Un salto que se comiera el retorno dejaría el programa
    /// en cualquier parte.
    #[test]
    fn despues_de_un_go_to_el_perform_vuelve() {
        let src = programa_con_parrafos(
            "01 X PIC 9.",
            "PERFORM 1000-A THRU 1000-FIN.\n\
             DISPLAY \"volvi\".\n\
             STOP RUN.\n\
             1000-A.\n\
             DISPLAY \"a\".\n\
             GO TO 1000-FIN.\n\
             1000-B.\n\
             DISPLAY \"b\".\n\
             1000-FIN.\n\
             EXIT.",
        );
        assert_eq!(run_cobol(&src), "a\nvolvi\n", "o no salto, o no volvio");
    }

    /// Un `GO TO` hacia ATRÁS es un bucle, y es COBOL legítimo del de siempre.
    #[test]
    fn un_go_to_hacia_atras_es_un_bucle() {
        let src = programa_con_parrafos(
            "01 I PIC 9(3) VALUE ZERO.",
            "PERFORM 1000-BUCLE THRU 1000-FIN.\n\
             DISPLAY I.\n\
             STOP RUN.\n\
             1000-BUCLE.\n\
             ADD 1 TO I.\n\
             IF I < 4\n\
             GO TO 1000-BUCLE\n\
             END-IF.\n\
             1000-FIN.\n\
             EXIT.",
        );
        assert_eq!(run_cobol(&src), "4\n");
    }

    /// Desde el cuerpo principal NO: aquí un párrafo es una subrutina a la que
    /// se entra por `call`, y saltar dentro sin haber entrado por su `PERFORM`
    /// dejaría el `ret` del final sin dirección a la que volver.
    #[test]
    fn un_go_to_desde_el_cuerpo_principal_se_rechaza() {
        let src = programa_con_parrafos(
            "01 X PIC 9.",
            "GO TO 1000-A.\nSTOP RUN.\n1000-A.\nDISPLAY \"a\".",
        );
        let err = compile_source_to_bef(&src).unwrap_err().to_string();
        assert!(err.contains("cuerpo principal"), "{err}");
    }

    /// Y a un párrafo que no existe, tampoco.
    #[test]
    fn un_go_to_a_la_nada_se_rechaza() {
        let src = programa_con_parrafos(
            "01 X PIC 9.",
            "PERFORM 1000-A.\nSTOP RUN.\n1000-A.\nGO TO 9000-NO-EXISTE.",
        );
        let err = compile_source_to_bef(&src).unwrap_err().to_string();
        assert!(err.contains("no hay ningun parrafo"), "{err}");
    }

    // ── ON SIZE ERROR: qué pasa cuando el resultado NO CABE ─────────────
    //
    // Sin la cláusula, COBOL guarda el número recortado por arriba y sigue. Con
    // ella, el campo **no se toca** y el programa decide.

    /// ★ LA PARTE QUE IMPORTA: cuando no cabe, **el destino se queda como
    /// estaba**. No es un tecnicismo — deja el saldo anterior intacto para que
    /// el programa lo pueda escribir en un informe de rechazos y seguir.
    #[test]
    fn on_size_error_no_toca_el_campo() {
        let src = program(
            "01 A PIC 9(3) VALUE 123.",
            "ADD 900 TO A ON SIZE ERROR\nDISPLAY \"no cabe\"\nEND-ADD.\nDISPLAY A.",
        );
        // 123 + 900 = 1023, y en tres dígitos no entra.
        assert_eq!(run_cobol(&src), "no cabe\n123\n", "el campo se toco igualmente");
    }

    /// ⚠ Y sin la cláusula, **BMO se queda con el número entero**: `1023` en un
    /// `PIC 9(3)`.
    ///
    /// Eso **no es lo que dice el estándar** —COBOL recorta por arriba y
    /// guardaría `023`— y es una divergencia conocida: un campo `DISPLAY` de
    /// BMO sigue siendo un entero de 64 bits y no mide lo que dice su PICTURE.
    /// Es la tarea `1.5` del plan, la única de la fase 1 que sigue abierta.
    ///
    /// Se fija aquí a propósito. El día que `1.5` entre, este test **tiene que
    /// cambiar**, y ése es justo el aviso que hace falta: un cambio de
    /// almacenamiento que altera resultados no puede pasar callando.
    ///
    /// Mientras tanto tiene una consecuencia buena: hoy `ON SIZE ERROR` es lo
    /// ÚNICO que caza un desbordamiento en BMO.
    #[test]
    fn sin_on_size_error_bmo_no_recorta_todavia() {
        let src = program("01 A PIC 9(3) VALUE 123.", "ADD 900 TO A.\nDISPLAY A.");
        assert_eq!(
            run_cobol(&src),
            "1023\n",
            "si esto da 023, es que 1.5 entro y hay que actualizar el plan"
        );
    }

    /// `NOT ON SIZE ERROR` — lo que se hace cuando SÍ cupo.
    #[test]
    fn not_on_size_error_corre_cuando_cabe() {
        let src = program(
            "01 A PIC 9(5) VALUE 123.\n01 N PIC 9(3) VALUE ZERO.",
            "ADD 900 TO A ON SIZE ERROR\nDISPLAY \"no cabe\"\n\
             NOT ON SIZE ERROR\nADD 1 TO N\nEND-ADD.\nDISPLAY A.\nDISPLAY N.",
        );
        assert_eq!(run_cobol(&src), "1023\n1\n");
    }

    /// ★ DIVIDIR ENTRE CERO es un desborde, no un fallo del CPU.
    ///
    /// Sin esto, el `idiv` levanta `#DE` y el proceso muere sin decir por qué.
    /// En un batch eso es peor que un número malo: se lleva por delante el
    /// proceso entero por culpa de un registro.
    #[test]
    fn dividir_entre_cero_es_un_desborde_y_no_una_muerte() {
        let src = program(
            "01 A PIC S9(7)V99 VALUE 100.00.\n01 D PIC 9(3) VALUE ZERO.",
            "DIVIDE D BY A ON SIZE ERROR\nDISPLAY \"division por cero\"\nEND-DIVIDE.\n\
             DISPLAY A.",
        );
        assert_eq!(run_cobol(&src), "division por cero\n100.00\n");
    }

    /// La cláusula vale en las cinco, no sólo en `ADD`.
    #[test]
    fn on_size_error_vale_en_las_cinco() {
        let casos: &[(&str, &str)] = &[
            ("01 A PIC 9(3) VALUE 999.", "ADD 999 TO A ON SIZE ERROR\nDISPLAY \"x\"\nEND-ADD."),
            ("01 A PIC 9(3) VALUE 100.", "SUBTRACT 9999 FROM A ON SIZE ERROR\nDISPLAY \"x\"\nEND-SUBTRACT."),
            ("01 A PIC 9(3) VALUE 999.", "MULTIPLY 999 BY A ON SIZE ERROR\nDISPLAY \"x\"\nEND-MULTIPLY."),
            ("01 A PIC 9(3) VALUE 100.", "DIVIDE 0 BY A ON SIZE ERROR\nDISPLAY \"x\"\nEND-DIVIDE."),
            ("01 A PIC 9(3) VALUE 1.", "COMPUTE A = 999 * 999 ON SIZE ERROR\nDISPLAY \"x\"\nEND-COMPUTE."),
        ];
        for (data, body) in casos {
            let src = program(data, body);
            assert_eq!(run_cobol(&src), "x\n", "no salto el desborde en: {body}");
        }
    }

    /// Un `SUBTRACT` que se pasa por abajo también desborda: `-9899` no cabe en
    /// un `PIC 9(3)`, y el signo no cambia la cuenta de dígitos.
    #[test]
    fn el_desborde_mira_la_magnitud_no_el_signo() {
        let src = program(
            "01 A PIC S9(3) VALUE 100.",
            "SUBTRACT 9999 FROM A ON SIZE ERROR\nDISPLAY \"no cabe\"\nEND-SUBTRACT.\nDISPLAY A.",
        );
        assert_eq!(run_cobol(&src), "no cabe\n100\n");
    }

    /// Sin `END-<verbo>` no se sabe dónde acaba la cláusula, y tragarse lo de
    /// después la convertiría en el resto del programa.
    #[test]
    fn una_clausula_sin_cierre_se_rechaza() {
        let src = program("01 A PIC 9(3).", "ADD 1 TO A ON SIZE ERROR\nDISPLAY \"x\".");
        let err = compile_source_to_bef(&src).unwrap_err().to_string();
        assert!(err.contains("END-ADD"), "{err}");
    }

    // ── INSPECT y STRING: manejo de texto ───────────────────────────────

    /// `TALLYING` — contar apariciones. En banca, lo más corriente es contar
    /// espacios para saber cuánto mide de verdad un campo que viene rellenado.
    #[test]
    fn inspect_tallying_cuenta_las_veces() {
        let src = program(
            "01 T PIC X(10) VALUE \"AB CD EF\".\n01 N PIC 9(3) VALUE ZERO.",
            "INSPECT T TALLYING N FOR ALL \" \".\nDISPLAY N.",
        );
        // "AB CD EF" son ocho letras; el campo es de diez, así que hay dos
        // espacios dentro y dos de relleno: cuatro.
        assert_eq!(run_cobol(&src), "4\n");
    }

    /// ★ `ALL` y `LEADING` NO son lo mismo, y sobre un importe es otro número.
    /// Ésta es la razón por la que hay dos formas y no una con una opción.
    #[test]
    fn all_y_leading_no_son_lo_mismo() {
        let con_all = program(
            "01 T PIC X(7) VALUE \"  12 34\".",
            "INSPECT T REPLACING ALL \" \" BY \"0\".\nDISPLAY T.",
        );
        assert_eq!(run_cobol(&con_all), "0012034\n");

        let con_leading = program(
            "01 T PIC X(7) VALUE \"  12 34\".",
            "INSPECT T REPLACING LEADING \" \" BY \"0\".\nDISPLAY T.",
        );
        assert_eq!(run_cobol(&con_leading), "0012 34\n", "LEADING paso del primer no-espacio");
    }

    /// El caso que trae medio fichero de intercambio: un importe con espacios
    /// delante que hay que rellenar de ceros.
    #[test]
    fn inspect_rellena_de_ceros_un_importe_con_espacios() {
        let src = program(
            "01 T PIC X(8) VALUE \"   12345\".",
            "INSPECT T REPLACING LEADING SPACE BY ZERO.\nDISPLAY T.",
        );
        assert_eq!(run_cobol(&src), "00012345\n");
    }

    /// `STRING … DELIMITED BY SIZE` — pegar campos y literales en orden.
    #[test]
    fn string_pega_campos_y_literales() {
        let src = program(
            "01 A PIC X(4) VALUE \"4471\".\n01 B PIC X(4) VALUE \"9982\".\n\
             01 C PIC X(9).",
            "STRING A DELIMITED BY SIZE\n\
             \"-\" DELIMITED BY SIZE\n\
             B DELIMITED BY SIZE\n\
             INTO C.\nDISPLAY C.",
        );
        assert_eq!(run_cobol(&src), "4471-9982\n");
    }

    /// Lo que sobra del destino queda a espacios — no con lo del `MOVE`
    /// anterior.
    ///
    /// ★ Este test cazó un fallo que no avisaba: la palabra `SIZE` de
    /// `DELIMITED BY SIZE` se colaba como si fuera un campo más, y sus dos
    /// primeras letras acababan escritas DENTRO del destino. Compilaba, no
    /// decía nada, y metía basura en un registro.
    #[test]
    fn string_no_se_sale_ni_deja_cola() {
        let src = program(
            "01 A PIC X(4) VALUE \"AAAA\".\n01 D PIC X(6).",
            "MOVE \"ZZZZZZ\" TO D.\n\
             STRING A DELIMITED BY SIZE INTO D.\nDISPLAY D.",
        );
        assert_eq!(run_cobol(&src), "AAAA  \n", "quedo cola, o se colo una palabra clave");
    }

    /// Lo que no se compila se dice con su motivo, y el motivo explica **qué
    /// pasaría** si se aceptara a medias.
    #[test]
    fn las_formas_de_texto_que_faltan_se_rechazan() {
        let casos: &[(&str, &str, &str)] = &[
            (
                "01 T PIC X(8).\n01 N PIC 9(3).",
                "INSPECT T TALLYING N FOR ALL \"AB\".",
                "busqueda de subcadena",
            ),
            (
                "01 T PIC X(8).",
                "INSPECT T CHARACTERS.",
                "TALLYING",
            ),
            (
                "01 A PIC X(4).\n01 C PIC X(9).",
                "STRING A DELIMITED BY SPACE INTO C.",
                "solo `DELIMITED BY SIZE`",
            ),
            (
                "01 N PIC 9(4).\n01 M PIC 9(3).",
                "INSPECT N TALLYING M FOR ALL \" \".",
                "campo de TEXTO",
            ),
        ];
        for (data, body, pista) in casos {
            let src = program(data, body);
            let err = compile_source_to_bef(&src)
                .expect_err(&format!("deberia rechazarse: {body}"))
                .to_string();
            assert!(err.contains(pista), "{body}\n => {err:?}");
        }
    }

    // ── FILE STATUS: lo que un batch mira después de CADA operación ─────
    //
    // No es ceremonia: un batch nocturno que revienta es peor que uno que
    // escribe "no pude abrir el maestro" y para ordenadamente.

    /// Un programa con UN fichero y su `FILE STATUS` declarado. El ayudante
    /// general no sirve: sus `SELECT` no lo llevan, y ése es justo el trozo que
    /// se está probando.
    fn programa_con_estado(decls: &str, body: &str) -> String {
        format!(
            "IDENTIFICATION DIVISION.
PROGRAM-ID. T.
             ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
             SELECT ENTRADA ASSIGN TO \"d/e.txt\" FILE STATUS IS ST.
             DATA DIVISION.
{decls}
PROCEDURE DIVISION.
{body}
STOP RUN.
"
        )
    }

    /// ★ `35` — el fichero no existe. Es el caso que más se da y el único
    /// motivo que la puerta permite distinguir hoy.
    #[test]
    fn file_status_dice_35_cuando_el_fichero_no_esta() {
        let src = programa_con_estado(
            "FILE SECTION.
FD ENTRADA.
01 R PIC 9(4).
             WORKING-STORAGE SECTION.
01 ST PIC XX VALUE \"??\".",
            "OPEN INPUT ENTRADA.
             IF ST = \"00\"
DISPLAY \"abierto\"
ELSE
DISPLAY ST
END-IF.",
        );
        // Sin sembrar el fichero: no existe.
        let (consola, _) = run_cobol_con_disco(&src, &[]);
        assert_eq!(consola, "35
");
    }

    /// Y `00` cuando sí está.
    #[test]
    fn file_status_dice_00_cuando_abre() {
        let src = programa_con_estado(
            "FILE SECTION.
FD ENTRADA.
01 R PIC 9(4).
             WORKING-STORAGE SECTION.
01 ST PIC XX VALUE \"??\".",
            "OPEN INPUT ENTRADA.
DISPLAY ST.",
        );
        let (consola, _) = run_cobol_con_disco(&src, &[("d/e.txt", "1234
")]);
        assert_eq!(consola, "00
");
    }

    /// ★ `10` — fin de fichero. Es la forma del estándar de escribir un bucle
    /// de batch: se lee hasta que el estado deja de ser `00`.
    #[test]
    fn file_status_dice_10_al_acabarse_el_fichero() {
        let src = programa_con_estado(
            "FILE SECTION.
FD ENTRADA.
01 IMPORTE PIC S9(7)V99.
             WORKING-STORAGE SECTION.
             01 ST PIC XX VALUE \"??\".
             01 TOTAL PIC S9(9)V99 VALUE ZERO.
             01 CUANTOS PIC 9(3) VALUE ZERO.",
            "OPEN INPUT ENTRADA.
             PERFORM UNTIL ST NOT = \"00\"
             READ ENTRADA
             AT END CONTINUE
             NOT AT END ADD IMPORTE TO TOTAL
             ADD 1 TO CUANTOS
             END-READ
             END-PERFORM.
             CLOSE ENTRADA.
             DISPLAY CUANTOS.
DISPLAY TOTAL.
DISPLAY ST.",
        );
        let (consola, _) = run_cobol_con_disco(&src, &[("d/e.txt", "100.00
25.50
0.50
")]);
        // Tres registros, y el bucle paró POR EL ESTADO y no por una bandera
        // puesta a mano. El CLOSE lo devuelve a `00`.
        assert_eq!(consola, "3
126.00
00
");
    }

    /// Un programa que ESCRIBE `d/s.txt` y mira su estado después del `CLOSE`.
    fn programa_que_guarda(body: &str) -> String {
        format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
             ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
             SELECT SALIDA ASSIGN TO \"d/s.txt\" FILE STATUS IS ST.\n\
             DATA DIVISION.\nFILE SECTION.\nFD SALIDA.\n01 R PIC 9(4).\n\
             WORKING-STORAGE SECTION.\n01 ST PIC XX VALUE \"??\".\n\
             PROCEDURE DIVISION.\n{body}\nSTOP RUN.\n"
        )
    }

    /// ★★ `30` — **el `CLOSE` que no guardó**, que es el estado que más
    /// importa de todos.
    ///
    /// Hasta el `CLOSE` no hay nada en el disco: escribir es un acto de dos
    /// pasos y el segundo es éste. `emit_close` ponía `"00"` a pelo sin mirar
    /// lo que contestaba la puerta, así que un programa que se había molestado
    /// en declarar `FILE STATUS` —o sea, uno que preguntaba— recibía "todo
    /// bien" con el fichero sin escribir.
    ///
    /// Y no es un caso de laboratorio: hoy `TASK_OP_ARCHIVO_CREAR` **no puede
    /// reemplazar un fichero que ya existe**, así que la SEGUNDA corrida de
    /// cualquier programa que escriba su salida cae exactamente aquí.
    #[test]
    fn file_status_dice_30_cuando_el_close_no_guarda() {
        let src = programa_que_guarda(
            "OPEN OUTPUT SALIDA.\nMOVE 1234 TO R.\nWRITE R.\nCLOSE SALIDA.\nDISPLAY ST.",
        );
        let (consola, m) = run_cobol_sin_poder_guardar(&src, &[], &["d/s.txt"]);
        assert_eq!(consola, "30\n", "el programa tiene que ENTERARSE de que no se guardo");
        // Y que no quede a medias: o entero o nada.
        assert_eq!(m.archivo("d/s.txt"), None, "no se puede guardar un trozo");
    }

    /// Y `00` con el mismo programa cuando el disco sí acepta.
    ///
    /// Es la mitad que impide que el arreglo de arriba sea "poner `30` siempre":
    /// las dos pruebas juntas dicen que el estado **depende de lo que pasó**.
    #[test]
    fn file_status_dice_00_cuando_el_close_guarda() {
        let src = programa_que_guarda(
            "OPEN OUTPUT SALIDA.\nMOVE 1234 TO R.\nWRITE R.\nCLOSE SALIDA.\nDISPLAY ST.",
        );
        let (consola, m) = run_cobol_sin_poder_guardar(&src, &[], &[]);
        assert_eq!(consola, "00\n");
        assert!(m.archivo("d/s.txt").is_some(), "esta vez si tiene que estar en el disco");
    }

    /// ★ Cerrar una ENTRADA no puede dar `30` por accidente.
    ///
    /// La puerta contesta `1` al cerrar un fichero de lectura porque no hay
    /// nada que guardar. Si eso se leyera como fallo, todo batch que cierre su
    /// fichero de entrada —o sea, todos— se pararía creyendo que el disco está
    /// roto. Ésta es la prueba de que el arreglo no se pasó de listo.
    #[test]
    fn cerrar_una_entrada_sigue_dando_00() {
        let src = programa_con_estado(
            "FILE SECTION.\nFD ENTRADA.\n01 R PIC 9(4).\n\
             WORKING-STORAGE SECTION.\n01 ST PIC XX VALUE \"??\".",
            "OPEN INPUT ENTRADA.\nCLOSE ENTRADA.\nDISPLAY ST.",
        );
        let (consola, _) = run_cobol_con_disco(&src, &[("d/e.txt", "1234\n")]);
        assert_eq!(consola, "00\n");
    }

    /// El campo tiene que existir y medir DOS letras. Si no, el programa
    /// compararía contra basura y decidiría por ella — `IF ST = "00"` daría
    /// falso siempre y el batch se pararía cada noche sin motivo.
    #[test]
    fn un_file_status_mal_declarado_se_rechaza() {
        let casos: &[(&str, &str)] = &[
            ("WORKING-STORAGE SECTION.\n01 OTRO PIC XX.", "no esta declarado"),
            ("WORKING-STORAGE SECTION.\n01 ST PIC X(5).", "tiene que ser `PIC XX`"),
            ("WORKING-STORAGE SECTION.\n01 ST PIC 99.", "tiene que ser `PIC XX`"),
        ];
        for (decls, pista) in casos {
            let src = format!(
                "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
                 ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
                 SELECT ENTRADA ASSIGN TO \"d/e.txt\" FILE STATUS IS ST.\n\
                 DATA DIVISION.\nFILE SECTION.\nFD ENTRADA.\n01 R PIC 9(4).\n\
                 {decls}\nPROCEDURE DIVISION.\nOPEN INPUT ENTRADA.\nSTOP RUN.\n"
            );
            let err = compile_source_to_bef(&src)
                .expect_err(&format!("deberia rechazarse: {decls}"))
                .to_string();
            assert!(err.contains(pista), "{decls}\n => {err:?}");
        }
    }

    // ── TEXTO: `PIC X(n)` con caracteres de verdad ──────────────────────
    //
    // Hasta aquí un `PIC X` reservaba sitio y se cargaba como un entero de 64
    // bits: no había campos de texto. Por eso `VALUE "HOLA"` se rechazaba.

    /// Lo mínimo: declarar, inicializar y enseñar.
    #[test]
    fn un_campo_de_texto_guarda_caracteres() {
        let src = program(
            "01 NOMBRE PIC X(10) VALUE \"BANCO BMO\".",
            "DISPLAY NOMBRE.",
        );
        // Diez caracteres: el nombre y un espacio de relleno.
        assert_eq!(run_cobol(&src), "BANCO BMO \n");
    }

    /// ★ El `VALUE` con ESPACIOS dentro. El troceado por espacios lo partía, y
    /// `VALUE "SIN SALDO"` guardaba `SIN` y leía el resto como cláusulas.
    #[test]
    fn un_value_de_texto_admite_espacios() {
        let src = program("01 T PIC X(12) VALUE \"SIN SALDO\".", "DISPLAY T.");
        assert_eq!(run_cobol(&src), "SIN SALDO   \n");
    }

    /// `MOVE` de literal y de campo a campo, con el relleno de espacios que
    /// manda el estándar.
    #[test]
    fn el_texto_se_mueve_y_se_rellena_con_espacios() {
        let src = program(
            "01 A PIC X(8).\n01 B PIC X(8).",
            "MOVE \"HOLA\" TO A.\nMOVE A TO B.\nDISPLAY B.",
        );
        assert_eq!(run_cobol(&src), "HOLA    \n");
    }

    /// ★ Y el relleno IMPORTA: un `MOVE` corto detrás de uno largo no puede
    /// dejar la cola del anterior. Un `FILE STATUS` que arrastra la letra de la
    /// operación de antes es peor que uno vacío.
    #[test]
    fn un_move_corto_borra_lo_que_habia_detras() {
        let src = program(
            "01 T PIC X(8).",
            "MOVE \"AAAAAAAA\" TO T.\nMOVE \"BB\" TO T.\nDISPLAY T.",
        );
        assert_eq!(run_cobol(&src), "BB      \n", "quedo cola del MOVE anterior");
    }

    /// ★ LA COMPARACIÓN, que es para lo que existe `FILE STATUS`.
    #[test]
    fn el_texto_se_compara_con_un_literal() {
        for (valor, esperado) in [("00", "bien\n"), ("10", "fin\n"), ("35", "otro\n")] {
            let src = program(
                "01 ST PIC XX.",
                &format!(
                    "MOVE \"{valor}\" TO ST.\n\
                     IF ST = \"00\"\nDISPLAY \"bien\"\n\
                     ELSE\nIF ST = \"10\"\nDISPLAY \"fin\"\n\
                     ELSE\nDISPLAY \"otro\"\nEND-IF\nEND-IF."
                ),
            );
            assert_eq!(run_cobol(&src), esperado, "estado {valor}");
        }
    }

    /// `NOT =`, y campo contra campo.
    #[test]
    fn el_texto_se_compara_de_las_dos_formas() {
        let src = program(
            "01 A PIC X(6).\n01 B PIC X(6).",
            "MOVE \"ABC\" TO A.\nMOVE \"ABC\" TO B.\n\
             IF A = B\nDISPLAY \"iguales\"\nEND-IF.\n\
             MOVE \"XYZ\" TO B.\n\
             IF A NOT = B\nDISPLAY \"distintos\"\nEND-IF.",
        );
        assert_eq!(run_cobol(&src), "iguales\ndistintos\n");
    }

    /// Un campo de más de ocho caracteres: la comparación recorre varios trozos
    /// y la diferencia puede estar en cualquiera.
    #[test]
    fn el_texto_largo_se_compara_entero() {
        let src = program(
            "01 T PIC X(20).",
            "MOVE \"4471998200000000000X\" TO T.\n\
             IF T = \"4471998200000000000X\"\nDISPLAY \"si\"\nEND-IF.\n\
             IF T = \"4471998200000000000Y\"\nDISPLAY \"mal\"\nELSE\nDISPLAY \"pillado\"\nEND-IF.",
        );
        assert_eq!(run_cobol(&src), "si\npillado\n", "la diferencia del ultimo trozo se perdio");
    }

    /// Lo que no se puede hacer se dice. Comparar cadenas por ORDEN depende del
    /// juego de caracteres, y decidirlo por ASCII a la callada daría un orden
    /// que no es el de un mainframe.
    #[test]
    fn el_texto_no_se_compara_por_orden_ni_se_mezcla_con_numeros() {
        let casos: &[(&str, &str, &str)] = &[
            ("01 A PIC X(4).\n01 B PIC X(4).", "IF A > B\nDISPLAY \"x\"\nEND-IF.", "juego de caracteres"),
            ("01 A PIC X(4).\n01 N PIC 9(4).", "MOVE A TO N.", "FUNCTION NUMVAL"),
        ];
        for (data, body, pista) in casos {
            let src = program(data, body);
            let err = compile_source_to_bef(&src)
                .expect_err(&format!("deberia rechazarse: {body}"))
                .to_string();
            assert!(err.contains(pista), "{body}\n => {err:?}");
        }
    }

    // ── REGISTROS BINARIOS: leer lo que ya existe ───────────────────────
    //
    // Hasta aquí el fichero era TEXTO: una línea, un número. Un banco no da
    // eso — da registros de largo fijo con los campos en su byte y los importes
    // empaquetados. Esto es `1.1` + `1.2` del plan.

    /// ★ EL VIAJE COMPLETO: escribir un registro binario y volver a leerlo.
    ///
    /// El fichero que queda **no es texto**: son 16 bytes por registro, sin
    /// salto de línea, con el número zonado y el importe en nibbles. Que salga
    /// y vuelva igual es lo que prueba que las dos mitades —empaquetar y
    /// desempaquetar— dicen lo mismo.
    #[test]
    fn un_registro_binario_va_al_disco_y_vuelve() {
        let src = programa_con_ficheros(
            "FILE SECTION.\n\
             FD SALIDA.\n\
             01 REG-OUT.\n\
             05 O-NUM PIC 9(10).\n\
             05 O-IMP PIC S9(7)V99 COMP-3.\n\
             05 O-EST PIC 9.\n\
             FD ENTRADA.\n\
             01 REG-IN.\n\
             05 I-NUM PIC 9(10).\n\
             05 I-IMP PIC S9(7)V99 COMP-3.\n\
             05 I-EST PIC 9.\n\
             WORKING-STORAGE SECTION.\n01 FIN PIC 9 VALUE ZERO.",
            "MOVE 4471998200 TO O-NUM.\nMOVE -1234.56 TO O-IMP.\nMOVE 7 TO O-EST.\n\
             OPEN OUTPUT SALIDA.\nWRITE REG-OUT.\nCLOSE SALIDA.",
        );
        let (_, m) = run_cobol_con_disco(&src, &[]);
        let bytes = m.archivo("d/s.txt").expect("tiene que haber fichero").to_vec();

        // ★ 16 bytes EXACTOS y ni uno más: 10 zonados + 5 empaquetados + 1.
        // Un salto de línea aquí correría todo lo de detrás.
        assert_eq!(bytes.len(), 16, "el registro no mide lo que dice su copybook");
        assert_eq!(&bytes[0..10], b"4471998200", "el numero no salio zonado");
        // -1234.56 en centavos = -123456, en 5 bytes: 00 01 23 45 6D
        assert_eq!(&bytes[10..15], &[0x00, 0x01, 0x23, 0x45, 0x6D]);
        assert_eq!(bytes[15], b'7');
    }

    /// Y el otro sentido, con **varios registros seguidos** — que es donde el
    /// resto de siete bytes de la puerta se nota. Con registros de 16 bytes, el
    /// primero deja sobra y el segundo la tiene que gastar antes de pedir más.
    #[test]
    fn un_batch_lee_registros_binarios_seguidos() {
        // Tres registros de 16: número zonado, importe empaquetado, estado.
        let mut datos: Vec<u8> = Vec::new();
        for (num, cent) in [(1u64, 1000_00i64), (2, 234_56), (3, -100_00)] {
            datos.extend_from_slice(format!("{num:010}").as_bytes());
            // El empaquetado a mano, para no probar el código con el código.
            let neg = cent < 0;
            let mut d = format!("{:09}", cent.abs());
            d.push(if neg { 'd' } else { 'c' });
            for par in d.as_bytes().chunks(2) {
                let alto = (par[0] - b'0') << 4;
                let bajo = if par[1] == b'c' { 0x0C } else if par[1] == b'd' { 0x0D }
                           else { par[1] - b'0' };
                datos.push(alto | bajo);
            }
            datos.push(b'0');
        }

        let src = programa_con_ficheros(
            "FILE SECTION.\n\
             FD ENTRADA.\n\
             01 REG-IN.\n\
             05 I-NUM PIC 9(10).\n\
             05 I-IMP PIC S9(7)V99 COMP-3.\n\
             05 I-EST PIC 9.\n\
             WORKING-STORAGE SECTION.\n\
             01 TOTAL PIC S9(9)V99 COMP-3 VALUE ZERO.\n\
             01 CUANTOS PIC 9(3) VALUE ZERO.\n\
             01 ULTIMO PIC 9(10) VALUE ZERO.\n\
             01 FIN PIC 9 VALUE ZERO.\n88 SE-ACABO VALUE 1.",
            "OPEN INPUT ENTRADA.\n\
             PERFORM UNTIL SE-ACABO\n\
             READ ENTRADA\n\
             AT END MOVE 1 TO FIN\n\
             NOT AT END ADD I-IMP TO TOTAL\n\
             ADD 1 TO CUANTOS\n\
             MOVE I-NUM TO ULTIMO\n\
             END-READ\n\
             END-PERFORM.\n\
             CLOSE ENTRADA.\n\
             DISPLAY CUANTOS.\nDISPLAY TOTAL.\nDISPLAY ULTIMO.",
        );
        let (consola, _) = run_cobol_con_disco_bytes(&src, &[("d/e.txt", &datos)]);
        // 1000.00 + 234.56 - 100.00 = 1134.56, y el último número es el 3.
        assert_eq!(consola, "3\n1134.56\n3\n", "los registros se corrieron o se perdio alguno");
    }

    /// ★★ EL VIAJE ENTERO: un programa COBOL escribe un fichero binario, y el
    /// VISOR lo lee y lo enseña.
    ///
    /// Es la prueba de que el visor **no puede mentir sobre lo que el programa
    /// escribió**: los dos usan la misma disposición, y los decodificadores del
    /// visor están comparados contra los emitidos en `bmo-lower`.
    ///
    /// Si alguien cambia el empaquetado sin tocar el visor, o al revés, este
    /// test lo dice — que es exactamente lo que un copybook mantenido a mano no
    /// puede hacer.
    #[test]
    fn el_visor_lee_lo_que_el_programa_escribio() {
        let src = programa_con_ficheros(
            "FILE SECTION.\n\
             FD SALIDA.\n\
             01 REG-CUENTA.\n\
             05 CTA-NUMERO PIC 9(10).\n\
             05 CTA-SALDO  PIC S9(7)V99 COMP-3.\n\
             05 CTA-ESTADO PIC 9.\n\
             WORKING-STORAGE SECTION.\n01 X PIC 9.",
            "OPEN OUTPUT SALIDA.\n\
             MOVE 4471998200 TO CTA-NUMERO.\nMOVE 15234.75 TO CTA-SALDO.\n\
             MOVE 1 TO CTA-ESTADO.\nWRITE REG-CUENTA.\n\
             MOVE 4471998201 TO CTA-NUMERO.\nMOVE -890.10 TO CTA-SALDO.\n\
             MOVE 2 TO CTA-ESTADO.\nWRITE REG-CUENTA.\n\
             CLOSE SALIDA.",
        );
        let (_, m) = run_cobol_con_disco(&src, &[]);
        let bytes = m.archivo("d/s.txt").expect("tiene que haber fichero").to_vec();
        assert_eq!(bytes.len(), 32, "dos registros de 16");

        let visto = ver_registros(&src, &bytes, Some("REG-CUENTA"), 10).unwrap();

        // Los importes, decodificados y con su coma puesta.
        assert!(visto.contains("2 registro(s) de 16"), "{visto}");
        assert!(visto.contains("4471998200"), "{visto}");
        assert!(visto.contains("15234.75"), "el saldo empaquetado no se leyo:\n{visto}");
        assert!(visto.contains("-890.10"), "el signo del segundo no se leyo:\n{visto}");
        // Y los bytes crudos al lado, que es lo que hace de esto un visor y no
        // un volcado de variables.
        // 15234.75 → 1523475 centavos → nueve dígitos `001523475` + signo `C`.
        assert!(visto.contains("00 15 23 47 5C"), "faltan los bytes crudos:\n{visto}");
    }

    /// ★ Un fichero que NO cuadra con el copybook. Es el síntoma clásico de
    /// estar mirando el formato equivocado, y callarlo dejaría al que mira
    /// creyendo que el último registro es raro.
    #[test]
    fn el_visor_avisa_cuando_el_fichero_no_cuadra() {
        let src = programa_con_ficheros(
            "FILE SECTION.\nFD ENTRADA.\n01 REG.\n05 A PIC 9(4).\n05 B PIC 9(4).\n\
             WORKING-STORAGE SECTION.\n01 X PIC 9.",
            "DISPLAY \"x\".",
        );
        // 20 bytes con registros de 8: sobran 4.
        let datos: Vec<u8> = b"1111222233334444abcd".to_vec();
        let visto = ver_registros(&src, &datos, None, 10).unwrap();
        assert!(visto.contains("SOBRAN 4 BYTES"), "{visto}");
        assert!(visto.contains("no es"), "{visto}");
        assert!(visto.contains("LO QUE SOBRA"), "{visto}");
        // Y aun así enseña los dos que sí cuadran.
        assert!(visto.contains("1111"), "{visto}");
    }

    // ── ROUNDED: el redondeo es una decisión LEGAL ──────────────────────
    //
    // No es una cláusula de sintaxis. Medio céntimo repetido cuatro millones de
    // veces es dinero de verdad, y hay jurisdicciones que obligan al redondeo
    // del banquero precisamente porque el clásico tiene sesgo.

    /// ★ EL CÉNTIMO. El 7,5 % de 133.33 son 9.99975 €.
    ///
    /// Sin `ROUNDED` se guarda 9.99; con `ROUNDED`, 10.00. **Ese céntimo es la
    /// razón por la que la cláusula existe**, y el test que prueba que aquí
    /// hace algo: si `ROUNDED` fuera decorativo, las dos líneas saldrían igual.
    #[test]
    fn rounded_cambia_el_centimo() {
        let sin = program(
            "01 BASE PIC S9(7)V99 VALUE 133.33.\n01 R PIC S9(7)V99.",
            "COMPUTE R = BASE * 0.075.\nDISPLAY R.",
        );
        assert_eq!(run_cobol(&sin), "9.99\n");

        let con = program(
            "01 BASE PIC S9(7)V99 VALUE 133.33.\n01 R PIC S9(7)V99.",
            "COMPUTE R ROUNDED = BASE * 0.075.\nDISPLAY R.",
        );
        assert_eq!(run_cobol(&con), "10.00\n", "ROUNDED no cambio nada");
    }

    /// ★ Un bug de precisión que este trabajo destapó, y que no era de
    /// `ROUNDED`: `COMPUTE` evaluaba TODO en la escala del destino, así que un
    /// literal con más decimales se recortaba **antes** de operar.
    ///
    /// `BASE * 0.075` con un destino de dos decimales multiplicaba por `0.07`.
    /// El resultado salía mal en el tercer decimal y ningún redondeo podía
    /// arreglarlo, porque para cuando llegaba el dígito ya no estaba.
    ///
    /// Ahora se calcula en la escala más alta que aparezca y se baja **una
    /// vez**, al final. Sin `ROUNDED` el resultado sigue truncándose — pero
    /// truncando el número bueno.
    #[test]
    fn compute_no_recorta_los_operandos_antes_de_operar() {
        // 133.33 × 0.075 = 9.99975. Con el fallo daba 9.33 (× 0.07).
        let src = program(
            "01 BASE PIC S9(7)V99 VALUE 133.33.\n01 R PIC S9(7)V99.",
            "COMPUTE R = BASE * 0.075.\nDISPLAY R.",
        );
        assert_eq!(run_cobol(&src), "9.99\n", "el literal se recorto antes de multiplicar");

        // Y con una variable de más decimales, no sólo con un literal.
        let src = program(
            "01 BASE PIC S9(7)V99 VALUE 100.00.\n01 TASA PIC S9V9(4) VALUE 0.0725.\n\
             01 R PIC S9(7)V99.",
            "COMPUTE R = BASE * TASA.\nDISPLAY R.",
        );
        assert_eq!(run_cobol(&src), "7.25\n");
    }

    /// El default de COBOL **sin** `ROUNDED` es TRUNCAR, y eso no es un
    /// descuido del estándar: en el desglose de un asiento hay que truncar para
    /// que la suma de las partes cuadre con el total.
    #[test]
    fn sin_rounded_se_trunca_y_es_a_proposito() {
        let src = program(
            "01 A PIC S9(7)V99 VALUE 100.00.",
            "DIVIDE 3 BY A.\nDISPLAY A.",
        );
        assert_eq!(run_cobol(&src), "33.33\n"); // 33.3333… truncado
    }

    /// ★ Los seis modos sobre el MISMO número, para que se vea que cada uno
    /// dice algo distinto y que la palabra del estándar llega hasta el CPU.
    ///
    /// `100.00 / 8 = 12.50` exacto en céntimos, así que se usa `/ 16`:
    /// `6.25` → a dos decimales no hay empate. Se toma `/ 3` y `/ 7`, que dan
    /// restos por los dos lados de la mitad.
    #[test]
    fn los_seis_modos_llegan_hasta_el_cpu() {
        // 10.00 / 3 = 3.3333… → el resto es 0.33…, por debajo de la mitad
        // 10.00 / 7 = 1.42857… → 1.4285…, y el dígito que decide es un 8
        let casos: &[(&str, &str, &str)] = &[
            ("", "3.33", "1.42"),                                    // sin ROUNDED
            ("ROUNDED", "3.33", "1.43"),                             // clásico
            ("ROUNDED MODE IS NEAREST-EVEN", "3.33", "1.43"),        // banquero
            ("ROUNDED MODE IS NEAREST-TOWARD-ZERO", "3.33", "1.43"),
            ("ROUNDED MODE IS TOWARD-GREATER", "3.34", "1.43"),      // techo
            ("ROUNDED MODE IS TOWARD-LESSER", "3.33", "1.42"),       // suelo
            ("ROUNDED MODE IS TRUNCATION", "3.33", "1.42"),
        ];
        for (clausula, esp3, esp7) in casos {
            for (divisor, esperado) in [("3", esp3), ("7", esp7)] {
                let src = program(
                    "01 A PIC S9(7)V99.",
                    &format!("MOVE 10.00 TO A.\nDIVIDE {divisor} BY A {clausula}.\nDISPLAY A."),
                );
                assert_eq!(
                    run_cobol(&src),
                    format!("{esperado}\n"),
                    "10.00 / {divisor} con `{clausula}`"
                );
            }
        }
    }

    /// ★ El SESGO del redondeo clásico, contado con dinero.
    ///
    /// Cuatro empates seguidos: con el clásico los cuatro suben y aparecen dos
    /// céntimos de la nada; con el del banquero, dos suben y dos bajan y la
    /// suma cuadra con la exacta. **Ése es el motivo por el que el modo existe,
    /// y por el que hay jurisdicciones que lo exigen.**
    #[test]
    fn el_sesgo_del_clasico_se_ve_en_cuatro_empates() {
        // 0.005, 0.015, 0.025, 0.035 sobre un campo de dos decimales.
        // La suma exacta es 0.08.
        let cuerpo = |clausula: &str| {
            format!(
                "MOVE 0 TO T.\n\
                 MOVE 0.005 TO X.\nADD X TO T {clausula}.\n\
                 MOVE 0.015 TO X.\nADD X TO T {clausula}.\n\
                 MOVE 0.025 TO X.\nADD X TO T {clausula}.\n\
                 MOVE 0.035 TO X.\nADD X TO T {clausula}.\n\
                 DISPLAY T."
            )
        };
        let datos = "01 T PIC S9(5)V99.\n01 X PIC S9(5)V9(3).";
        // Clásico: 0.01 + 0.02 + 0.03 + 0.04 = 0.10 — dos céntimos de más.
        assert_eq!(run_cobol(&program(datos, &cuerpo("ROUNDED"))), "0.10\n");
        // Banquero: 0.00 + 0.02 + 0.02 + 0.04 = 0.08 — cuadra con la suma exacta.
        assert_eq!(
            run_cobol(&program(datos, &cuerpo("ROUNDED MODE IS NEAREST-EVEN"))),
            "0.08\n",
            "el redondeo del banquero tiene que cuadrar con la suma exacta"
        );
    }

    /// `ROUNDED` en las cinco aritméticas, no sólo en `COMPUTE`.
    #[test]
    fn rounded_vale_en_las_cinco() {
        // ADD de un literal con más decimales de los que caben.
        let src = program("01 A PIC S9(5)V99 VALUE ZERO.", "ADD 1.005 TO A ROUNDED.\nDISPLAY A.");
        assert_eq!(run_cobol(&src), "1.01\n");

        let src = program("01 A PIC S9(5)V99 VALUE ZERO.", "ADD 1.005 TO A.\nDISPLAY A.");
        assert_eq!(run_cobol(&src), "1.00\n", "sin ROUNDED tiene que truncar");

        // SUBTRACT: 10.00 − 1.005 = 8.995, y `.995` sube. El resultado se
        // redondea DESPUÉS de restar, no antes: si se redondeara el `1.005` a
        // `1.01` primero, saldría 8.99.
        let src = program("01 A PIC S9(5)V99 VALUE 10.00.", "SUBTRACT 1.005 FROM A ROUNDED.\nDISPLAY A.");
        assert_eq!(run_cobol(&src), "9.00\n");

        // MULTIPLY: 3.33 × 3.003 = 10.00 (9.99999) — el operando se carga en
        // SU escala, así que los tres decimales del 3.003 cuentan.
        let src = program(
            "01 A PIC S9(5)V99 VALUE 3.33.\n01 B PIC S9(5)V9(3) VALUE 3.003.",
            "MULTIPLY B BY A ROUNDED.\nDISPLAY A.",
        );
        assert_eq!(run_cobol(&src), "10.00\n");

        // DIVIDE.
        let src = program("01 A PIC S9(5)V99 VALUE 10.00.", "DIVIDE 3 BY A ROUNDED.\nDISPLAY A.");
        assert_eq!(run_cobol(&src), "3.33\n");
    }

    /// El signo. `-9.995` con el clásico va **lejos del cero**: `-10.00`.
    /// Redondear hacia arriba un descubierto lo haría más pequeño de lo que es.
    #[test]
    fn rounded_respeta_el_signo() {
        let src = program(
            "01 A PIC S9(5)V99 VALUE ZERO.",
            "SUBTRACT 9.995 FROM A ROUNDED.\nDISPLAY A.",
        );
        assert_eq!(run_cobol(&src), "-10.00\n");

        let src = program(
            "01 A PIC S9(5)V99 VALUE ZERO.",
            "SUBTRACT 9.995 FROM A ROUNDED MODE IS TOWARD-GREATER.\nDISPLAY A.",
        );
        assert_eq!(run_cobol(&src), "-9.99\n", "el techo de -9.995 es -9.99");
    }

    /// Lo que no es un modo del estándar se dice, con la lista de los que sí.
    #[test]
    fn los_modos_inventados_se_rechazan() {
        let casos: &[(&str, &str)] = &[
            ("COMPUTE A ROUNDED MODE IS HACIA-ARRIBA = 1.", "no es un modo del estandar"),
            ("COMPUTE A ROUNDED MODE IS PROHIBITED = 1.", "PROHIBITED no se compila"),
        ];
        for (body, pista) in casos {
            let src = program("01 A PIC S9(5)V99.", body);
            let err = compile_source_to_bef(&src)
                .expect_err(&format!("deberia rechazarse: {body}"))
                .to_string();
            assert!(err.contains(pista), "{body}\n => {err:?}");
        }
    }

    // ── EVALUATE: el verbo que más falta hacía ──────────────────────────
    //
    // Estaba marcado en el plan como bloqueado por el parser de tokens. Era
    // falso: `parser.rs` ya consume varias líneas para `IF … END-IF`, y
    // `EVALUATE … WHEN … END-EVALUATE` tiene la misma forma.

    /// La forma clásica: un sujeto y sus valores.
    #[test]
    fn evaluate_con_sujeto_elige_la_rama() {
        for tipo in 1..=4 {
            let esperado = match tipo {
                1 => "cargo\n",
                2 => "abono\n",
                3 => "comision\n",
                _ => "desconocido\n",
            };
            let src = program(
                "01 TIPO PIC 9.",
                &format!(
                    "MOVE {tipo} TO TIPO.\n\
                     EVALUATE TIPO\n\
                     WHEN 1\nDISPLAY \"cargo\"\n\
                     WHEN 2\nDISPLAY \"abono\"\n\
                     WHEN 3\nDISPLAY \"comision\"\n\
                     WHEN OTHER\nDISPLAY \"desconocido\"\n\
                     END-EVALUATE."
                ),
            );
            assert_eq!(run_cobol(&src), esperado, "tipo {tipo}");
        }
    }

    /// ★ La OTRA forma, y la que un banco usa para un escalado: `EVALUATE TRUE`
    /// con una condición entera por rama. Es la **tabla de decisión**.
    ///
    /// El orden manda: la primera que acierta gana, y las de abajo ni se
    /// prueban. Por eso los tramos se escriben de mayor a menor y `1500` tiene
    /// que dar `alta` aunque también cumpla las dos de abajo.
    #[test]
    fn evaluate_true_es_una_tabla_de_decision() {
        let casos: &[(&str, &str)] = &[
            ("1500.00", "alta\n"),
            ("1000.01", "alta\n"),
            ("1000.00", "media\n"),
            ("100.01", "media\n"),
            ("100.00", "baja\n"),
            ("0.00", "baja\n"),
        ];
        for (saldo, esperado) in casos {
            let src = program(
                "01 SALDO PIC S9(7)V99.",
                &format!(
                    "MOVE {saldo} TO SALDO.\n\
                     EVALUATE TRUE\n\
                     WHEN SALDO > 1000.00\nDISPLAY \"alta\"\n\
                     WHEN SALDO > 100.00\nDISPLAY \"media\"\n\
                     WHEN OTHER\nDISPLAY \"baja\"\n\
                     END-EVALUATE."
                ),
            );
            assert_eq!(run_cobol(&src), *esperado, "saldo {saldo}");
        }
    }

    /// ★ `WHEN 2 THRU 5` y `WHEN 6, 7` — la misma expansión que un nivel 88.
    ///
    /// Es lo que se gana compartiendo `Condicion::de_valores`: el `THRU` y la
    /// coma funcionaron aquí sin escribir una línea de gramática nueva.
    #[test]
    fn un_when_admite_rangos_y_listas() {
        for dia in 0..=9 {
            let esperado = match dia {
                1 => "lunes\n",
                2..=5 => "entre semana\n",
                6 | 7 => "fin de semana\n",
                _ => "no es un dia\n",
            };
            let src = program(
                "01 DIA PIC 9.",
                &format!(
                    "MOVE {dia} TO DIA.\n\
                     EVALUATE DIA\n\
                     WHEN 1\nDISPLAY \"lunes\"\n\
                     WHEN 2 THRU 5\nDISPLAY \"entre semana\"\n\
                     WHEN 6, 7\nDISPLAY \"fin de semana\"\n\
                     WHEN OTHER\nDISPLAY \"no es un dia\"\n\
                     END-EVALUATE."
                ),
            );
            assert_eq!(run_cobol(&src), esperado, "dia {dia}");
        }
    }

    /// Sin `WHEN OTHER`, si no acierta ninguna no pasa nada — y sobre todo, se
    /// sigue por la línea de abajo. Un `EVALUATE` que se comiera el resto del
    /// programa cuando no acierta sería un agujero silencioso.
    #[test]
    fn un_evaluate_sin_other_no_se_come_lo_que_viene_despues() {
        let src = program(
            "01 T PIC 9.",
            "MOVE 9 TO T.\n\
             EVALUATE T\n\
             WHEN 1\nDISPLAY \"uno\"\n\
             WHEN 2\nDISPLAY \"dos\"\n\
             END-EVALUATE.\n\
             DISPLAY \"sigo\".",
        );
        assert_eq!(run_cobol(&src), "sigo\n");
    }

    /// Varias sentencias por rama, y sólo las de la rama que gana.
    #[test]
    fn una_rama_puede_tener_varias_sentencias() {
        let src = program(
            "01 T PIC 9.\n01 A PIC S9(7)V99 VALUE ZERO.",
            "MOVE 2 TO T.\n\
             EVALUATE T\n\
             WHEN 1\nADD 100.00 TO A\nDISPLAY \"uno\"\n\
             WHEN 2\nADD 19.99 TO A\nADD 19.99 TO A\nDISPLAY \"dos\"\n\
             END-EVALUATE.\n\
             DISPLAY A.",
        );
        assert_eq!(run_cobol(&src), "dos\n39.98\n");
    }

    /// Un `EVALUATE` DENTRO de un párrafo, con `PERFORM` en las ramas: es como
    /// se escribe el despacho de un batch de verdad.
    #[test]
    fn un_evaluate_despacha_a_parrafos() {
        let src = programa_con_parrafos(
            "01 T PIC 9.\n01 CARGOS PIC 9(3) VALUE ZERO.\n01 ABONOS PIC 9(3) VALUE ZERO.",
            "MOVE 2 TO T.\n\
             PERFORM 1000-DESPACHA.\n\
             DISPLAY CARGOS.\n\
             DISPLAY ABONOS.\n\
             STOP RUN.\n\
             1000-DESPACHA.\n\
             EVALUATE T\n\
             WHEN 1\nPERFORM 2000-CARGO\n\
             WHEN 2\nPERFORM 3000-ABONO\n\
             END-EVALUATE.\n\
             2000-CARGO.\n\
             ADD 1 TO CARGOS.\n\
             3000-ABONO.\n\
             ADD 1 TO ABONOS.",
        );
        assert_eq!(run_cobol(&src), "0\n1\n");
    }

    /// Anidado, que es donde un emisor con una sola etiqueta de fin se rompe.
    #[test]
    fn los_evaluate_se_anidan() {
        let src = program(
            "01 A PIC 9.\n01 B PIC 9.",
            "MOVE 1 TO A.\nMOVE 2 TO B.\n\
             EVALUATE A\n\
             WHEN 1\n\
             EVALUATE B\n\
             WHEN 1\nDISPLAY \"1-1\"\n\
             WHEN 2\nDISPLAY \"1-2\"\n\
             END-EVALUATE\n\
             WHEN 2\nDISPLAY \"2\"\n\
             END-EVALUATE.\n\
             DISPLAY \"fin\".",
        );
        assert_eq!(run_cobol(&src), "1-2\nfin\n");
    }

    /// Y un `88` como condición de un `EVALUATE TRUE`, que es lo que hace que
    /// una tabla de decisión se lea en voz alta.
    #[test]
    fn un_evaluate_true_admite_nombres_de_condicion() {
        let src = program(
            "01 DIA PIC 9.\n88 LABORABLE VALUE 1 THRU 5.\n88 FESTIVO VALUE 6, 7.",
            "MOVE 6 TO DIA.\n\
             EVALUATE TRUE\n\
             WHEN LABORABLE\nDISPLAY \"abre\"\n\
             WHEN FESTIVO\nDISPLAY \"cierra\"\n\
             WHEN OTHER\nDISPLAY \"no existe\"\n\
             END-EVALUATE.",
        );
        assert_eq!(run_cobol(&src), "cierra\n");
    }

    /// Lo que no se compila se dice, y lo que no se alcanzaría nunca también.
    #[test]
    fn los_evaluate_mal_escritos_se_rechazan() {
        let casos: &[(&str, &str)] = &[
            (
                "EVALUATE T\nWHEN 1\nDISPLAY \"a\"\n",
                "END-EVALUATE",
            ),
            (
                "EVALUATE T\nEND-EVALUATE.",
                "sin ningun WHEN",
            ),
            (
                "EVALUATE T\nDISPLAY \"suelto\"\nWHEN 1\nDISPLAY \"a\"\nEND-EVALUATE.",
                "entre el EVALUATE y el primer WHEN",
            ),
            (
                "EVALUATE T\nWHEN OTHER\nDISPLAY \"o\"\nWHEN 1\nDISPLAY \"a\"\nEND-EVALUATE.",
                "el OTHER va el ultimo",
            ),
            (
                "EVALUATE T ALSO U\nWHEN 1\nDISPLAY \"a\"\nEND-EVALUATE.",
                "Varios sujetos",
            ),
            (
                "EVALUATE FALSE\nWHEN 1\nDISPLAY \"a\"\nEND-EVALUATE.",
                "EVALUATE FALSE no se compila",
            ),
        ];
        for (body, pista) in casos {
            let src = program("01 T PIC 9.\n01 U PIC 9.", body);
            let err = compile_source_to_bef(&src)
                .expect_err(&format!("deberia rechazarse: {body}"))
                .to_string();
            assert!(err.contains(pista), "{body}\n => {err:?}\n  (se esperaba {pista:?})");
        }
    }

    // ── PÁRRAFOS: la estructura de todo COBOL real ──────────────────────
    //
    // Un programa era una lista plana de sentencias. Un batch de banca no se
    // escribe así: se escribe con un cuerpo principal de cinco `PERFORM`
    // legibles y el trabajo repartido en párrafos con nombre.

    /// La forma corriente: cuerpo principal con `PERFORM`, `STOP RUN`, y los
    /// párrafos detrás.
    #[test]
    fn el_perform_de_parrafo_llama_y_vuelve() {
        let src = programa_con_parrafos(
            "01 A PIC 9(3).",
            "MOVE 0 TO A.\n\
             PERFORM 1000-SUMA.\n\
             DISPLAY A.\n\
             STOP RUN.\n\
             1000-SUMA.\n\
             ADD 5 TO A.",
        );
        assert_eq!(run_cobol(&src), "5\n");
    }

    /// El orden importa y se ve: si el `PERFORM` no volviera, la segunda línea
    /// no saldría.
    #[test]
    fn despues_del_perform_sigue_el_cuerpo_principal() {
        let src = programa_con_parrafos(
            "01 A PIC 9(3).",
            "PERFORM 1000-UNO.\n\
             DISPLAY \"vuelvo\".\n\
             STOP RUN.\n\
             1000-UNO.\n\
             DISPLAY \"dentro\".",
        );
        assert_eq!(run_cobol(&src), "dentro\nvuelvo\n");
    }

    /// ★ `PERFORM A THRU C` ejecuta A, B y C — **todo lo que hay entre los
    /// dos**, porque están seguidos en el código.
    ///
    /// Es la prueba de que el epílogo de cada párrafo pregunta en ejecución en
    /// vez de retornar siempre. Un emisor que pusiera un `ret` fijo al final de
    /// cada párrafo pasaría el test de arriba y fallaría éste.
    #[test]
    fn un_perform_thru_recorre_todos_los_parrafos_del_rango() {
        let src = programa_con_parrafos(
            "01 A PIC 9(3).",
            "PERFORM 1000-A THRU 3000-C.\n\
             DISPLAY \"fin\".\n\
             STOP RUN.\n\
             1000-A.\n\
             DISPLAY \"a\".\n\
             2000-B.\n\
             DISPLAY \"b\".\n\
             3000-C.\n\
             DISPLAY \"c\".",
        );
        assert_eq!(run_cobol(&src), "a\nb\nc\nfin\n");
    }

    /// Y el MISMO párrafo, llamado solo, no arrastra al siguiente. Es la otra
    /// mitad de lo mismo: si el rango se decidiera al compilar, uno de los dos
    /// tests tendría que fallar.
    #[test]
    fn el_mismo_parrafo_llamado_solo_no_arrastra_al_siguiente() {
        let src = programa_con_parrafos(
            "01 A PIC 9(3).",
            "PERFORM 1000-A.\n\
             DISPLAY \"fin\".\n\
             STOP RUN.\n\
             1000-A.\n\
             DISPLAY \"a\".\n\
             2000-B.\n\
             DISPLAY \"b\".",
        );
        assert_eq!(run_cobol(&src), "a\nfin\n", "1000-A se llevo por delante a 2000-B");
    }

    /// Un `PERFORM` DENTRO de un párrafo. La salida del de fuera se guarda en
    /// la pila; sin eso, el de fuera no volvería nunca.
    #[test]
    fn los_perform_se_anidan() {
        let src = programa_con_parrafos(
            "01 A PIC 9(3).",
            "PERFORM 1000-FUERA.\n\
             DISPLAY \"raiz\".\n\
             STOP RUN.\n\
             1000-FUERA.\n\
             DISPLAY \"fuera-antes\".\n\
             PERFORM 2000-DENTRO.\n\
             DISPLAY \"fuera-despues\".\n\
             2000-DENTRO.\n\
             DISPLAY \"dentro\".",
        );
        assert_eq!(
            run_cobol(&src),
            "fuera-antes\ndentro\nfuera-despues\nraiz\n",
            "un PERFORM anidado se comio la salida del de fuera"
        );
    }

    /// ★ `PERFORM <párrafo> UNTIL <cond>` — **el bucle de un batch**: el
    /// párrafo lee y el `UNTIL` mira si se acabó.
    #[test]
    fn un_perform_de_parrafo_until_repite() {
        let src = programa_con_parrafos(
            "01 I PIC 9(3).",
            "MOVE 0 TO I.\n\
             PERFORM 1000-CUENTA UNTIL I = 4.\n\
             DISPLAY I.\n\
             STOP RUN.\n\
             1000-CUENTA.\n\
             ADD 1 TO I.",
        );
        assert_eq!(run_cobol(&src), "4\n");
    }

    /// `PERFORM <párrafo> <n> TIMES`.
    #[test]
    fn un_perform_de_parrafo_n_veces() {
        let src = programa_con_parrafos(
            "01 A PIC S9(7)V99.",
            "MOVE 0 TO A.\n\
             PERFORM 1000-CUOTA 3 TIMES.\n\
             DISPLAY A.\n\
             STOP RUN.\n\
             1000-CUOTA.\n\
             ADD 19.99 TO A.",
        );
        assert_eq!(run_cobol(&src), "59.97\n");
    }

    /// La otra forma corriente de escribirlo: **todo** en párrafos, sin cuerpo
    /// principal. Entonces el programa empieza por el primero.
    #[test]
    fn un_programa_que_empieza_por_un_parrafo() {
        let src = programa_con_parrafos(
            "01 A PIC 9(3).",
            "1000-PRINCIPAL.\n\
             MOVE 7 TO A.\n\
             PERFORM 2000-ENSENA.\n\
             STOP RUN.\n\
             2000-ENSENA.\n\
             DISPLAY A.",
        );
        assert_eq!(run_cobol(&src), "7\n");
    }

    /// `EXIT` no hace nada, y ese es su trabajo: ser el destino de un
    /// `PERFORM … THRU X-SALIR`.
    #[test]
    fn exit_es_el_final_de_un_rango_y_no_hace_nada() {
        let src = programa_con_parrafos(
            "01 A PIC 9(3).",
            "PERFORM 1000-A THRU 1000-SALIR.\n\
             DISPLAY \"fin\".\n\
             STOP RUN.\n\
             1000-A.\n\
             DISPLAY \"trabajo\".\n\
             1000-SALIR.\n\
             EXIT.",
        );
        assert_eq!(run_cobol(&src), "trabajo\nfin\n");
    }

    /// ★ UN BATCH ENTERO escrito como se escribe de verdad: cuerpo principal
    /// legible de tres `PERFORM`, y cada paso en su párrafo.
    ///
    /// Es la forma del 90 % del COBOL que hay escrito, y hasta hoy no compilaba
    /// ni una línea de ella.
    #[test]
    fn el_batch_con_parrafos_es_legible_y_cuadra() {
        let src = ficheros_con_parrafos(
            "FILE SECTION.\nFD ENTRADA.\n01 IMPORTE PIC S9(7)V99 COMP-3.\n\
             WORKING-STORAGE SECTION.\n\
             01 TOTAL PIC S9(9)V99 COMP-3 VALUE ZERO.\n\
             01 CUANTOS PIC 9(5) VALUE ZERO.\n\
             01 FIN PIC 9 VALUE ZERO.\n\
             88 SE-ACABO VALUE 1.",
            "PERFORM 1000-INICIO.\n\
             PERFORM 2000-PROCESO UNTIL SE-ACABO.\n\
             PERFORM 3000-CIERRE.\n\
             STOP RUN.\n\
             1000-INICIO.\n\
             DISPLAY \"CIERRE DEL DIA\".\n\
             OPEN INPUT ENTRADA.\n\
             2000-PROCESO.\n\
             READ ENTRADA\n\
             AT END MOVE 1 TO FIN\n\
             NOT AT END ADD IMPORTE TO TOTAL\n\
             END-READ.\n\
             3000-CIERRE.\n\
             CLOSE ENTRADA.\n\
             DISPLAY TOTAL.",
        );
        let (consola, _) =
            run_cobol_con_disco(&src, &[("d/e.txt", "1000.00\n234.56\n0.44\n-100.00\n")]);
        assert_eq!(consola, "CIERRE DEL DIA\n1135.00\n");
    }

    /// ★ EL EJEMPLO DE NIVEL 8, ejecutado entero: el batch escrito con
    /// párrafos, que es como está escrito el 90 % del COBOL que hay.
    ///
    /// Junta todo lo de la fase 0: `VALUE` que inicializa, `OR` en las
    /// condiciones, `88` colgando de un dato, `PERFORM … UNTIL <88>`,
    /// `PERFORM … THRU` sobre tres párrafos, `COMP-3` y una PIC editada.
    #[test]
    fn el_ejemplo_de_parrafos_cierra_el_dia() {
        let (salida, _) = run_cobol_con_disco(
            include_str!("../examples/8-parrafos/cierre.cob"),
            // El 0.00 de en medio es el que ejercita el descarte.
            &[("datos/movim.txt", "1000.00\n234.56\n0.00\n0.44\n-100.00\n")],
        );
        let esperado = [
            "BANCO BMO - CIERRE DEL DIA",
            "--------------------------",
            "movimientos contados:",
            "4", // los cinco menos el de cero
            "de mas de 500:",
            "1",
            "abonos:",
            "1",
            "total del dia:",
            " $1,135.00",
        ]
        .map(|l| format!("{l}\n"))
        .concat();
        assert_eq!(salida, esperado);
    }

    /// ★ EL NIVEL 9, ejecutado: la tabla de decisión y el redondeo legal.
    #[test]
    fn el_ejemplo_de_decision_calcula_las_comisiones() {
        let salida = run_cobol(include_str!("../examples/9-decision/comision.cob"));
        // Tres clientes, tres tramos. 1500 × 0,25 % = 3,75 exacto.
        assert!(salida.contains(" $1,500.00"), "{salida}");
        assert!(salida.contains("     $3.75"), "el tramo preferente:\n{salida}");
        // 500 × 0,50 % = 2,50 exacto.
        assert!(salida.contains("     $2.50"), "{salida}");
        // 50 × 0,75 % = 0,375 → redondeado, 0,38. Truncado sería 0,37.
        assert!(salida.contains("     $0.38"), "el ROUNDED del tercer tramo:\n{salida}");
        // ★ Y el sesgo: el clásico inventa dos céntimos, el banquero cuadra.
        assert!(salida.contains("0.10"), "el clasico tenia que subir los cuatro:\n{salida}");
        assert!(salida.contains("0.08"), "el del banquero tenia que cuadrar:\n{salida}");
    }

    /// ★★ EL NIVEL 10: escribe el maestro binario y lo vuelve a leer.
    ///
    /// Que los importes salgan iguales prueba que empaquetar y desempaquetar
    /// dicen lo mismo. Y el fichero que queda mide 48 bytes exactos: tres
    /// registros de dieciséis, sin separador.
    #[test]
    fn el_ejemplo_binario_escribe_el_maestro_y_lo_relee() {
        let (salida, m) = run_cobol_con_disco(
            include_str!("../examples/10-binario/maestro.cob"),
            &[],
        );
        let bytes = m.archivo("datos/ctas.bin").expect("tiene que haber maestro");
        assert_eq!(bytes.len(), 48, "tres registros de 16, sin salto de linea");

        assert!(salida.contains("escritas 3 cuentas"), "{salida}");
        // Los tres saldos, releídos del disco: los importes empaquetados
        // vuelven iguales que como se escribieron.
        assert!(salida.contains("15,234.75"), "{salida}");
        assert!(salida.contains("3,105.40"), "{salida}");
        // ★ Y el que está en rojo sale con su `CR`. Con una máscara sin símbolo
        // de signo saldría `890.10` a secas y el extracto diría que la cuenta
        // está en verde — el fallo que este ejemplo existe para no cometer.
        assert!(salida.contains("890.10CR"), "el descubierto salio SIN signo:\n{salida}");
        // El cuadre: 15234.75 - 890.10 + 3105.40 = 17450.05
        assert!(salida.contains("17,450.05"), "el total no cuadra:\n{salida}");
        assert!(salida.contains("en descubierto:\n1\n"), "{salida}");
    }

    /// Lo que no cuadra se dice, en vez de saltar a cualquier parte.
    #[test]
    fn los_perform_de_parrafo_imposibles_se_rechazan() {
        let casos: &[(&str, &str)] = &[
            (
                "PERFORM 9000-NO-EXISTE.\nSTOP RUN.\n1000-A.\nDISPLAY \"a\".",
                "no hay ningun parrafo con ese nombre",
            ),
            (
                "PERFORM 2000-B THRU 1000-A.\nSTOP RUN.\n1000-A.\nDISPLAY \"a\".\n2000-B.\nDISPLAY \"b\".",
                "el final esta ANTES del principio",
            ),
            (
                "PERFORM 1000-A 3 TIMES UNTIL 1 = 1.\nSTOP RUN.\n1000-A.\nDISPLAY \"a\".",
                "hay que elegir una",
            ),
        ];
        for (body, pista) in casos {
            let src = programa_con_parrafos("01 A PIC 9(3).", body);
            let err = compile_source_to_bef(&src)
                .expect_err(&format!("deberia rechazarse: {body}"))
                .to_string();
            assert!(err.contains(pista), "{body}\n => {err:?}\n  (se esperaba {pista:?})");
        }
    }

    /// Dos párrafos con el mismo nombre hacen que un `PERFORM` no sepa a cuál
    /// va. Se dice al declararlos, no al llamarlos.
    #[test]
    fn dos_parrafos_con_el_mismo_nombre_se_rechazan() {
        let src = programa_con_parrafos(
            "01 A PIC 9(3).",
            "STOP RUN.\n1000-A.\nDISPLAY \"a\".\n1000-A.\nDISPLAY \"otra\".",
        );
        let err = compile_source_to_bef(&src).unwrap_err().to_string();
        assert!(err.contains("ya existe"), "{err}");
    }

    // ── VALUE: el valor con el que arranca un dato ──────────────────────
    //
    // Se parseaba desde siempre y no se emitía nunca. Un campo declarado con
    // VALUE arrancaba con lo que hubiera en la pila, y ningún ejemplo lo
    // destapaba porque todos inicializan a mano con MOVE.

    /// Sin un solo `MOVE`: el dato ya vale lo que dice su declaración.
    #[test]
    fn value_inicializa_el_dato() {
        let src = program(
            "01 SALDO PIC S9(7)V99 VALUE 100.50.\n01 CUANTOS PIC 9(3) VALUE 7.",
            "DISPLAY SALDO.\nDISPLAY CUANTOS.",
        );
        assert_eq!(run_cobol(&src), "100.50\n7\n");
    }

    /// El signo del valor inicial. Una cuenta que arranca en descubierto no
    /// puede arrancar en verde.
    #[test]
    fn value_conserva_el_signo() {
        let src = program("01 A PIC S9(5)V99 VALUE -1234.56.", "DISPLAY A.");
        assert_eq!(run_cobol(&src), "-1234.56\n");
    }

    /// `ZERO` / `ZEROS` / `ZEROES` es lo que escribe todo el mundo, y `VALUE 0`
    /// casi nadie. Las tres son la misma cosa.
    #[test]
    fn value_acepta_las_figurativas_del_cero() {
        for forma in ["ZERO", "ZEROS", "ZEROES", "0"] {
            let src = program(
                &format!("01 A PIC S9(5)V99 VALUE {forma}."),
                "ADD 1.25 TO A.\nDISPLAY A.",
            );
            assert_eq!(run_cobol(&src), "1.25\n", "forma {forma}");
        }
    }

    /// ★ Un `VALUE` sobre un `COMP-3` tiene que quedar EMPAQUETADO, no como un
    /// entero crudo en el hueco. Se ve porque el campo trunca a su PICTURE: si
    /// la inicialización se hubiera saltado el empaquetado, saldrían los cinco
    /// dígitos.
    #[test]
    fn value_sobre_comp3_queda_empaquetado() {
        let src = program("01 A PIC 9(3) COMP-3 VALUE 12345.", "DISPLAY A.");
        assert_eq!(run_cobol(&src), "345\n");
    }

    /// El estándar dice que un `VALUE` sobre una tabla llena **todas** las
    /// casillas, no la primera.
    #[test]
    fn value_sobre_una_tabla_llena_todas_las_casillas() {
        let src = program(
            "01 TABLA.\n05 T PIC S9(5)V99 VALUE 9.99 OCCURS 3 TIMES.",
            "DISPLAY T(1).\nDISPLAY T(2).\nDISPLAY T(3).",
        );
        assert_eq!(run_cobol(&src), "9.99\n9.99\n9.99\n");
    }

    /// El `VALUE` se pone ANTES de la primera sentencia, así que un `MOVE`
    /// posterior manda. Al revés —inicializar al final— borraría lo que el
    /// programa acaba de calcular.
    #[test]
    fn un_move_posterior_gana_al_value() {
        let src = program("01 A PIC 9(5) VALUE 111.", "MOVE 222 TO A.\nDISPLAY A.");
        assert_eq!(run_cobol(&src), "222\n");
    }

    /// Lo que no se puede guardar se dice, en vez de guardar otra cosa.
    #[test]
    fn los_value_que_no_se_pueden_guardar_se_rechazan() {
        let casos: &[(&str, &str)] = &[
            // `VALUE "HOLA"` sobre un `PIC X` ya NO está aquí: desde que existe
            // el texto (0.7), se guarda como caracteres. Sobre un campo
            // NUMÉRICO sigue sin tener sentido, y eso es lo que queda.
            ("01 A PIC 9(3) VALUE \"HOLA\".", "eso no es un numero"),
            ("01 A PIC 9(3) VALUE SPACES.", "eso no es un numero"),
            ("01 A VALUE 5.", "VALUE sin PIC"),
        ];
        for (decl, pista) in casos {
            let src = program(decl, "DISPLAY \"x\".");
            let err = compile_source_to_bef(&src)
                .expect_err(&format!("{decl} deberia rechazarse"))
                .to_string();
            assert!(err.contains(pista), "{decl} => {err:?}\n  (se esperaba {pista:?})");
        }
    }

    // ── COMP-3: el formato en el que están los datos de un banco ────────
    //
    // La trampa de esta característica es que se puede fingir entera: guardar
    // el mismo entero de siempre, no empaquetar nada, y todos los programas
    // seguirían dando el mismo resultado. Compilaría, validaría, y el día que
    // alguien leyera un fichero de verdad no habría nibbles donde tocaba.
    //
    // Por eso estas pruebas no comprueban que "el COMP-3 no rompe": comprueban
    // lo que SÓLO puede pasar si el dato de verdad vive empaquetado en un campo
    // del ancho que dice su PICTURE. Los bytes exactos están probados aparte,
    // en `bmo_lower::packed`.

    /// El decimal exacto sobrevive al empaquetado. Es lo mínimo: si tres cuotas
    /// de 19.99 dejaran de dar 59.97 al pasar por nibbles, el formato no
    /// serviría para lo único para lo que existe.
    #[test]
    fn comp3_mantiene_el_decimal_exacto() {
        let src = program(
            "01 SALDO PIC S9(7)V99 COMP-3.\n01 CUOTA PIC S9(5)V99 COMP-3.",
            "MOVE 0 TO SALDO.\nMOVE 19.99 TO CUOTA.\n\
             PERFORM 3 TIMES\nADD CUOTA TO SALDO\nEND-PERFORM.\n\
             DISPLAY SALDO.\n\
             IF SALDO = 59.97\nDISPLAY \"cuadra\"\nELSE\nDISPLAY \"se perdio\"\nEND-IF.",
        );
        assert_eq!(run_cobol(&src), "59.97\ncuadra\n");
    }

    /// ★ La prueba de que el almacenamiento es DE VERDAD el del PICTURE.
    ///
    /// Un `PIC 9(3)` COMP-3 son dos bytes: tres huecos de dígito y el signo. Lo
    /// que no cabe se pierde por arriba, que es lo que manda el estándar. El
    /// mismo dato en DISPLAY hoy sigue siendo un registro de 64 bits y guarda
    /// los cinco dígitos — así que este test falla en cuanto alguien convierta
    /// el COMP-3 en decoración.
    #[test]
    fn comp3_ocupa_lo_que_dice_su_picture_y_trunca() {
        let empaquetado = program("01 A PIC 9(3) COMP-3.", "MOVE 12345 TO A.\nDISPLAY A.");
        assert_eq!(run_cobol(&empaquetado), "345\n", "un COMP-3 de 3 digitos guardo mas de 3");

        let suelto = program("01 A PIC 9(3).", "MOVE 12345 TO A.\nDISPLAY A.");
        assert_eq!(run_cobol(&suelto), "12345\n", "el DISPLAY dejo de ser un entero de 64 bits");
    }

    /// El signo va en el último nibble, y tiene que volver. Un campo con `S`
    /// que perdiera el signo convertiría un cargo en un abono.
    #[test]
    fn comp3_conserva_el_signo() {
        let src = program(
            "01 A PIC S9(5)V99 COMP-3.",
            "MOVE 0 TO A.\nSUBTRACT 123.45 FROM A.\nDISPLAY A.",
        );
        assert_eq!(run_cobol(&src), "-123.45\n");
    }

    /// Y un campo SIN `S` guarda el valor absoluto, que es lo que dice el
    /// estándar. No es un detalle: es la diferencia entre un campo que puede
    /// estar en rojo y uno que no, y el fichero de al lado lo lee por el nibble.
    #[test]
    fn comp3_sin_signo_guarda_el_valor_absoluto() {
        let src = program(
            "01 A PIC 9(5)V99 COMP-3.",
            "MOVE 0 TO A.\nSUBTRACT 123.45 FROM A.\nDISPLAY A.",
        );
        assert_eq!(run_cobol(&src), "123.45\n");
    }

    /// Dos campos empaquetados seguidos no se pisan. Un `off by one` al
    /// escribir nibbles mete el importe de uno en el otro, y eso en un batch
    /// aparece como un descuadre semanas después.
    #[test]
    fn dos_comp3_seguidos_no_se_pisan() {
        let src = program(
            "01 A PIC S9(7)V99 COMP-3.\n01 B PIC S9(7)V99 COMP-3.\n01 C PIC S9(3) COMP-3.",
            "MOVE 11111.11 TO A.\nMOVE 22222.22 TO B.\nMOVE 333 TO C.\n\
             DISPLAY A.\nDISPLAY B.\nDISPLAY C.",
        );
        assert_eq!(run_cobol(&src), "11111.11\n22222.22\n333\n");
    }

    /// El empaquetado convive con lo que ya había: se puede mezclar un COMP-3
    /// con un DISPLAY en la misma cuenta, porque la aritmética sigue viendo el
    /// entero escalado y no la representación.
    #[test]
    fn comp3_y_display_se_mezclan_en_la_misma_cuenta() {
        let src = program(
            "01 P PIC S9(7)V99 COMP-3.\n01 D PIC 9(5)V99.\n01 R PIC S9(7)V99 COMP-3.",
            "MOVE 100.50 TO P.\nMOVE 25.25 TO D.\nCOMPUTE R = P + D.\nDISPLAY R.",
        );
        assert_eq!(run_cobol(&src), "125.75\n");
    }

    /// Una TABLA de empaquetados: cada elemento tiene sus propios nibbles.
    #[test]
    fn una_tabla_de_comp3_guarda_cada_casilla_aparte() {
        let src = program(
            "01 TABLA.\n05 T PIC S9(5)V99 COMP-3 OCCURS 3 TIMES.\n01 I PIC 9(2).",
            "MOVE 10.01 TO T(1).\nMOVE 20.02 TO T(2).\nMOVE 3 TO I.\nMOVE 30.03 TO T(I).\n\
             DISPLAY T(1).\nDISPLAY T(2).\nDISPLAY T(3).",
        );
        assert_eq!(run_cobol(&src), "10.01\n20.02\n30.03\n");
    }

    /// Un COMP-3 en el REGISTRO de un fichero: se lee del disco como texto y se
    /// guarda empaquetado. El fichero sigue siendo texto —los registros
    /// binarios son otro paso— pero el campo en memoria ya es el de un banco.
    #[test]
    fn el_registro_de_un_fichero_puede_ser_comp3() {
        let src = programa_con_ficheros(
            "FILE SECTION.\nFD ENTRADA.\n01 R PIC S9(7)V99 COMP-3.\n\
             WORKING-STORAGE SECTION.\n01 TOTAL PIC S9(9)V99 COMP-3.\n01 FIN PIC 9.",
            "MOVE 0 TO TOTAL.\nMOVE 0 TO FIN.\nOPEN INPUT ENTRADA.\n\
             PERFORM UNTIL FIN = 1\n\
             READ ENTRADA\nAT END MOVE 1 TO FIN\nNOT AT END ADD R TO TOTAL\nEND-READ\n\
             END-PERFORM.\nCLOSE ENTRADA.\nDISPLAY TOTAL.",
        );
        let (consola, _) = run_cobol_con_disco(&src, &[("d/e.txt", "19.99\n25.01\n0.50\n")]);
        assert_eq!(consola, "45.50\n");
    }

    /// ★ EL EJEMPLO DE NIVEL 7, ejecutado entero.
    ///
    /// Las dos primeras líneas de números son la prueba que no se puede
    /// fingir: el mismo `12345` en un campo empaquetado de tres dígitos y en
    /// uno sin empaquetar. Salen distintos porque el empaquetado mide lo que
    /// dice su PICTURE. El día que salgan iguales, el COMP-3 volvió a ser un
    /// entero con otro nombre.
    #[test]
    fn el_ejemplo_de_empaquetado_hace_lo_que_dice() {
        let (salida, _) = run_cobol_con_disco(
            include_str!("../examples/7-empaquetado/cuentas.cob"),
            &[("datos/movim.txt", "1000.00\n234.56\n0.44\n-100.00\n")],
        );
        let esperado = [
            "CUENTAS - DECIMAL EMPAQUETADO",
            "empaquetado de 3 digitos:",
            "345",
            "el mismo dato sin empaquetar:",
            "12345",
            "una cuenta en rojo:",
            "-1234.56",
            "el mismo importe en un campo sin signo:",
            "1234.56",
            "saldo tras el cierre, menos comision:",
            " $1,133.50",
        ]
        .map(|l| format!("{l}\n"))
        .concat();
        assert_eq!(salida, esperado);
    }

    /// Las cuatro formas de escribirlo son la misma cosa.
    #[test]
    fn comp3_se_escribe_de_cuatro_maneras() {
        for forma in ["COMP-3", "COMPUTATIONAL-3", "USAGE COMP-3", "USAGE IS PACKED-DECIMAL"] {
            let src = program(
                &format!("01 A PIC 9(3) {forma}."),
                "MOVE 12345 TO A.\nDISPLAY A.",
            );
            assert_eq!(run_cobol(&src), "345\n", "forma {forma}");
        }
    }

    /// Lo que NO se compila se dice CON SU MOTIVO. Aceptar `COMP` y guardar un
    /// entero de 64 bits sería compilar una palabra que promete un formato y no
    /// lo da — que es exactamente el fallo del que este compilador huye.
    #[test]
    fn los_usage_que_no_estan_se_rechazan_diciendo_por_que() {
        let casos: &[(&str, &str)] = &[
            ("01 A PIC 9(3) COMP.", "binario"),
            ("01 A PIC 9(3) BINARY.", "binario"),
            ("01 A PIC 9(3) COMP-5.", "binario"),
            ("01 A COMP-2.", "FLOTANTE"),
            ("01 A PIC 9(3)V99 COMP-1.", "FLOTANTE"),
            ("01 A COMP-3.", "sin PIC"),
            ("01 A PIC X(10) COMP-3.", "solo se empaqueta lo numerico"),
            ("01 A PIC $$$,$$9.99 COMP-3.", "es para ENSENAR"),
        ];
        for (decl, pista) in casos {
            let src = program(decl, "DISPLAY \"x\".");
            let err = compile_source_to_bef(&src)
                .expect_err(&format!("{decl} deberia rechazarse"))
                .to_string();
            assert!(err.contains(pista), "{decl} => {err:?}\n  (se esperaba que dijera {pista:?})");
        }
    }


    /// El payload `hola_COBOL.bex` que el kernel EMBEBE, ejecutado.
    ///
    /// Regenerar tras tocar el codegen:
    ///   cargo run -p bmo-cobol-front --     ///     toolchain/lang/cobol/examples/2-decimal/hola_COBOL.cob     ///     -o Ultra_kernel_x86-64/kernel/src/ring0/hola_COBOL.bex
    #[test]
    fn hola_cobol_payload_output_is_what_the_kernel_will_show() {
        let out = run_cobol(include_str!("../examples/2-decimal/hola_COBOL.cob"));
        let esperado = [
            "hola desde COBOL en el Ryzen",
            "3 x 19.99 = 59.97 exacto",
            "cargo entero aplicado bien",
            "recibo emitido",
            "recibo emitido",
            "dos devoluciones aplicadas",
            "COBOL termino ok",
        ]
        .map(|l| format!("{l}\n"))
        .concat();
        assert_eq!(out, esperado);
    }



    /// El extracto entero, ejecutado. Es la prueba de que la cadena completa
    /// —fuente COBOL, parser, codegen, BEF, CPU— produce la linea que un
    /// banco imprimiria, y no una aproximacion.
    ///
    /// Cada columna esta alineada porque cada campo mide lo que su PIC
    /// declara. Si alguien rompe el ancho, este test lo dice antes de que un
    /// informe salga descuadrado.
    #[test]
    fn el_extracto_imprime_las_lineas_de_un_banco() {
        let out = run_cobol(include_str!("../examples/3-presentacion/extracto.cob"));
        let esperado = [
            "BANCO BMO - EXTRACTO DE CUENTA",
            "-----------------------------",
            "saldo disponible:",
            "$12,345.67",
            "talon a cobrar:",
            "*****0.45",
            "balance final:",
            "  120.00CR",
            "cuenta en descubierto",
        ]
        .map(|l| format!("{l}\n"))
        .concat();
        assert_eq!(out, esperado);
    }

    /// ★ EL BATCH. Lee un fichero de movimientos, los totaliza en decimal
    /// exacto y escribe el cierre en otro fichero.
    ///
    /// Es el programa que justifica todo lo demás: hasta ahora BMO COBOL sabía
    /// calcular y sabía presentar, y no tenía de dónde sacar los datos.
    #[test]
    fn el_batch_totaliza_un_fichero_y_escribe_el_cierre() {
        let (salida, m) = run_cobol_con_disco(
            include_str!("../examples/4-ficheros/batch.cob"),
            // Cuatro movimientos. 1000.00 + 234.56 + 0.44 + (-100.00).
            &[("datos/movim.txt", "1000.00\n234.56\n0.44\n-100.00\n")],
        );
        let esperado = [
            "BATCH DE CIERRE - BANCO BMO",
            "total del dia:",
            " $1,135.00",
            "cierre escrito en datos/cierre.txt",
        ]
        .map(|l| format!("{l}\n"))
        .concat();
        assert_eq!(salida, esperado);
        // Y en el disco queda el total, no un fichero vacío ni a medias.
        assert_eq!(m.archivo_texto("datos/cierre.txt").as_deref(), Some("1135.00\n"));
    }

    /// Un fichero que no existe NO es un fichero vacío: el `AT END` salta a la
    /// primera y el total es cero, sin reventar. En un batch nocturno eso es
    /// la diferencia entre "hoy no hubo movimientos" y una caída.
    #[test]
    fn un_fichero_que_falta_da_cero_y_no_revienta() {
        let (salida, m) = run_cobol_con_disco(include_str!("../examples/4-ficheros/batch.cob"), &[]);
        assert!(salida.contains("total del dia:"), "{salida}");
        assert!(salida.contains("     $0.00"), "{salida}");
        assert_eq!(m.archivo_texto("datos/cierre.txt").as_deref(), Some("0.00\n"));
    }

    /// El último registro cuenta aunque el fichero no acabe en salto de línea.
    /// Es el clásico que se come el movimiento de más valor: el último.
    #[test]
    fn el_ultimo_registro_cuenta_sin_salto_final() {
        let (salida, _) = run_cobol_con_disco(
            include_str!("../examples/4-ficheros/batch.cob"),
            &[("datos/movim.txt", "10.00\n5.50")],
        );
        assert!(salida.contains("    $15.50"), "{salida}");
    }

    /// Los ficheros escritos desde el anfitrión traen `\r\n`. Ese `\r` dentro
    /// del número lo convertiría en otro.
    #[test]
    fn el_batch_aguanta_los_finales_de_windows() {
        let (salida, _) = run_cobol_con_disco(
            include_str!("../examples/4-ficheros/batch.cob"),
            &[("datos/movim.txt", "1000.00\r\n234.56\r\n")],
        );
        assert!(salida.contains(" $1,234.56"), "{salida}");
    }

    /// ★ EL CIERRE POR CONCEPTO. `OCCURS` y File I/O juntos: dos ficheros en
    /// paralelo, cada importe a la casilla de su concepto, y el informe con
    /// máscara.
    ///
    /// Es para esto que existe `OCCURS`: sin él harían falta `TOTAL-1`…
    /// `TOTAL-4` y el mismo `IF` cuatro veces. Y el subíndice viene **de un
    /// fichero**, o sea que la comprobación de rango no es teórica: la decide
    /// el dato, no el programador.
    #[test]
    fn el_cierre_por_concepto_totaliza_en_su_casilla() {
        let (salida, _) = run_cobol_con_disco(
            include_str!("../examples/5-tablas/conceptos.cob"),
            &[
                ("datos/concs.txt", "1\n3\n2\n3\n1\n"),
                ("datos/imps.txt", "100.00\n50.00\n25.50\n10.00\n5.00\n"),
            ],
        );
        let esperado = [
            "CIERRE POR CONCEPTO - BANCO BMO",
            "totales por concepto:",
            // 100.00 + 5.00 · 25.50 · 50.00 + 10.00 · nada
            "   $105.00",
            "    $25.50",
            "    $60.00",
            "     $0.00",
        ]
        .map(|l| format!("{l}\n"))
        .concat();
        assert_eq!(salida, esperado);
    }

    /// Y si el concepto que trae el fichero se sale de la tabla, el programa
    /// **para diciendo cuál** en vez de sumar en la casilla del vecino.
    #[test]
    fn un_concepto_fuera_de_la_tabla_para_el_cierre() {
        let (salida, _) = run_cobol_con_disco(
            include_str!("../examples/5-tablas/conceptos.cob"),
            &[("datos/concs.txt", "1\n7\n"), ("datos/imps.txt", "100.00\n50.00\n")],
        );
        assert!(
            salida.contains("SUBINDICE FUERA DE RANGO EN TOTAL-CONCEPTO (1..4)"),
            "{salida}"
        );
        assert!(!salida.contains("totales por concepto"), "no debe seguir: {salida}");
    }

    /// ★ LA CARTERA. El mismo batch escrito con nombres en vez de números:
    /// `PERFORM UNTIL SE-ACABO` y `IF NO-HUBO-NADA`.
    ///
    /// Es el nivel 88 haciendo lo único que hace: que la condición se lea en
    /// voz alta. Quien audite esto no tiene que acordarse de qué significaba
    /// el 1.
    #[test]
    fn la_cartera_reparte_cobros_y_devoluciones() {
        let (salida, _) = run_cobol_con_disco(
            include_str!("../examples/6-condiciones/cartera.cob"),
            &[("datos/movim.txt", "1000.00\n234.56\n-100.00\n0.44\n-50.00\n")],
        );
        let esperado = [
            "CARTERA DEL DIA - BANCO BMO",
            "cobros:",
            " $1,235.00",
            "devoluciones:",
            // `CR` y no un menos: una máscara sin signo se come el negativo —
            // correcto según el estándar, y mentira en un informe. Escribirlo
            // así fue el error de quien montó este test, no del compilador.
            "   $150.00CR",
        ]
        .map(|l| format!("{l}\n"))
        .concat();
        assert_eq!(salida, esperado);
    }

    /// Y sin movimientos lo DICE, en vez de imprimir ceros callando. En un
    /// cierre nocturno, un fichero vacío y uno que no se pudo leer se parecen
    /// demasiado si los dos dan cero.
    #[test]
    fn la_cartera_sin_movimientos_lo_dice_con_su_nombre() {
        let (salida, _) =
            run_cobol_con_disco(include_str!("../examples/6-condiciones/cartera.cob"), &[]);
        assert!(salida.contains("sin movimientos hoy"), "{salida}");
        assert!(!salida.contains("cobros:"), "no debe imprimir el informe: {salida}");
    }

    /// Un `READ` sin `AT END` se RECHAZA. Compilaría a un `PERFORM UNTIL` que
    /// no termina nunca, y eso es peor que no compilar.
    #[test]
    fn un_read_sin_at_end_se_rechaza() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
                   ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
                   SELECT F ASSIGN TO \"a.txt\".\n\
                   DATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC 9(3).\n\
                   PROCEDURE DIVISION.\nOPEN INPUT F.\nREAD F END-READ.\nCLOSE F.\nSTOP RUN.\n";
        let e = compile_source_to_bef(src).unwrap_err();
        assert!(format!("{e:?}").contains("AT END"), "{e:?}");
    }

    /// Una ruta que no cabe en 8.3 se rechaza AL COMPILAR.
    ///
    /// En la máquina, `apps/movimientos.txt` daría handle nulo, y COBOL lee un
    /// handle nulo como "fin de fichero desde el principio": un cierre a cero
    /// sin una sola queja. El nombre se sabe al compilar, así que se dice al
    /// compilar.
    #[test]
    fn una_ruta_que_no_cabe_en_8_3_se_rechaza_al_compilar() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
                   ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
                   SELECT F ASSIGN TO \"apps/movimientos.txt\".\n\
                   DATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC 9(3).\n\
                   PROCEDURE DIVISION.\nSTOP RUN.\n";
        let t = format!("{:?}", compile_source_to_bef(src).unwrap_err());
        assert!(t.contains("no cabe en 8.3") && t.contains("movimientos.txt"), "{t}");
    }

    /// Y las rutas que sí caben siguen pasando, incluida la letra de unidad.
    #[test]
    fn las_rutas_de_8_3_con_letra_de_unidad_pasan() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
                   ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
                   SELECT F ASSIGN TO \"A:/apps/movim.txt\".\n\
                   DATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC 9(3).\n\
                   PROCEDURE DIVISION.\nSTOP RUN.\n";
        assert!(compile_source_to_bef(src).is_ok(), "A:/apps/movim.txt tiene que valer");
    }

    /// Usar un fichero que nadie declaró se rechaza con el `SELECT` que falta,
    /// no con un "no se pudo".
    #[test]
    fn un_fichero_sin_select_se_rechaza_diciendo_cual() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
                   DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC 9(3).\n\
                   PROCEDURE DIVISION.\nOPEN INPUT NADIE.\nSTOP RUN.\n";
        let e = compile_source_to_bef(src).unwrap_err();
        let t = format!("{e:?}");
        assert!(t.contains("NADIE") && t.contains("SELECT"), "{t}");
    }

    // ── NIVEL 88: lo que se RECHAZA ─────────────────────────────────────

    /// Un `88` con `PIC` no es un 88: es alguien que cree estar declarando un
    /// dato. Se dice qué es un nombre de condición.
    #[test]
    fn un_88_con_pic_se_rechaza() {
        let src = program("01 F PIC 9.\n88 FIN PIC 9 VALUE 1.", "MOVE 1 TO F.");
        let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
        assert!(t.contains("nombre de condicion") && t.contains("no lleva PIC"), "{t}");
    }

    /// Sin `VALUE` no compara nada. Antes de existir el 88, esto habría sido un
    /// dato sin PIC con un nombre suelto.
    #[test]
    fn un_88_sin_value_se_rechaza() {
        let src = program("01 F PIC 9.\n88 FIN.", "MOVE 1 TO F.");
        let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
        assert!(t.contains("necesita su VALUE"), "{t}");
    }

    /// Un `88` es el apodo de una comparación sobre el dato de arriba. Si no
    /// hay nadie arriba, no hay de qué colgarlo.
    #[test]
    fn un_88_sin_dato_encima_se_rechaza() {
        let src = program("88 FIN VALUE 1.", "STOP RUN.");
        let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
        assert!(t.contains("no hay ningun dato encima"), "{t}");
    }

    /// ★ `88 … VALUE 1 THRU 5` — los dos extremos INCLUIDOS.
    ///
    /// Estaba rechazado porque expandirlo pide un `OR`. Ya hay `OR`, así que se
    /// expande a `DIA >= 1 AND DIA <= 5` y baja por el mismo emisor de árboles
    /// que una condición escrita a mano.
    ///
    /// Se recorre el rango entero y **los dos vecinos de fuera**: un `>` donde
    /// va un `>=` sólo se ve en el extremo, y ahí es donde vive el error de
    /// "el día 1 no era laborable".
    #[test]
    fn un_88_con_rango_compara_el_rango_entero() {
        for dia in 0..=7 {
            let esperado = if (1..=5).contains(&dia) { "labor\n" } else { "fiesta\n" };
            let src = program(
                "01 DIA PIC 9.\n88 LABORABLE VALUE 1 THRU 5.",
                &format!(
                    "MOVE {dia} TO DIA.\n\
                     IF LABORABLE\nDISPLAY \"labor\"\nELSE\nDISPLAY \"fiesta\"\nEND-IF."
                ),
            );
            assert_eq!(run_cobol(&src), esperado, "dia {dia}");
        }
    }

    /// Y varios valores sueltos, que es un `OR`. `THROUGH` es el sinónimo largo
    /// de `THRU` y tiene que valer igual.
    #[test]
    fn un_88_con_varios_valores_es_un_or() {
        for dia in 1..=7 {
            let esperado = if dia == 6 || dia == 7 { "fin\n" } else { "no\n" };
            let src = program(
                "01 DIA PIC 9.\n88 FIN-DE-SEMANA VALUE 6, 7.",
                &format!(
                    "MOVE {dia} TO DIA.\n\
                     IF FIN-DE-SEMANA\nDISPLAY \"fin\"\nELSE\nDISPLAY \"no\"\nEND-IF."
                ),
            );
            assert_eq!(run_cobol(&src), esperado, "dia {dia}");
        }

        let src = program(
            "01 D PIC 9.\n88 R VALUE 2 THROUGH 4.",
            "MOVE 3 TO D.\nIF R\nDISPLAY \"si\"\nELSE\nDISPLAY \"no\"\nEND-IF.",
        );
        assert_eq!(run_cobol(&src), "si\n", "THROUGH no vale lo mismo que THRU");
    }

    /// Mezclando las dos formas, que es como se escribe una tabla de códigos de
    /// verdad: unos sueltos y un tramo.
    #[test]
    fn un_88_mezcla_rangos_y_valores_sueltos() {
        for c in 0..=9 {
            let esperado = if c == 0 || (3..=5).contains(&c) || c == 9 { "si\n" } else { "no\n" };
            let src = program(
                "01 C PIC 9.\n88 VALIDO VALUE 0, 3 THRU 5, 9.",
                &format!("MOVE {c} TO C.\nIF VALIDO\nDISPLAY \"si\"\nELSE\nDISPLAY \"no\"\nEND-IF."),
            );
            assert_eq!(run_cobol(&src), esperado, "codigo {c}");
        }
    }

    /// Un `88` con decimales: el rango se compara en la escala del padre, no en
    /// enteros. Un `9.99` que se leyera como `9` daría un rango de más.
    #[test]
    fn un_88_con_rango_respeta_la_escala_del_padre() {
        let casos: &[(&str, &str)] = &[
            ("9.98", "fuera\n"),
            ("9.99", "dentro\n"),
            ("15.00", "dentro\n"),
            ("20.00", "dentro\n"),
            ("20.01", "fuera\n"),
        ];
        for (importe, esperado) in casos {
            let src = program(
                "01 IMPORTE PIC S9(5)V99.\n88 EN-TRAMO VALUE 9.99 THRU 20.00.",
                &format!(
                    "MOVE {importe} TO IMPORTE.\n\
                     IF EN-TRAMO\nDISPLAY \"dentro\"\nELSE\nDISPLAY \"fuera\"\nEND-IF."
                ),
            );
            assert_eq!(run_cobol(&src), *esperado, "importe {importe}");
        }
    }

    /// Un `88` dentro de una condición compuesta: se combina con lo demás como
    /// cualquier comparación, porque baja por el mismo árbol.
    #[test]
    fn un_88_se_combina_con_otras_condiciones() {
        let src = program(
            "01 D PIC 9.\n88 LABORABLE VALUE 1 THRU 5.\n01 SALDO PIC S9(5)V99.",
            "MOVE 3 TO D.\nMOVE 100.00 TO SALDO.\n\
             IF LABORABLE AND SALDO > 50.00\nDISPLAY \"abre\"\nELSE\nDISPLAY \"cierra\"\nEND-IF.",
        );
        assert_eq!(run_cobol(&src), "abre\n");
    }

    /// Una palabra suelta en un `IF` que no es ningún 88 se rechaza diciendo
    /// las dos salidas. Antes, `IF LO-QUE-SEA` no encontraba operador y el
    /// mensaje mandaba a buscar un `=` que nadie quería escribir.
    #[test]
    fn un_nombre_de_condicion_que_no_existe_se_rechaza() {
        let src = program("01 F PIC 9.", "MOVE 1 TO F.\nIF PEPE\nDISPLAY \"x\"\nEND-IF.");
        let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
        assert!(t.contains("PEPE") && t.contains("nivel 88"), "{t}");
    }

    /// Y un `88` no ocupa memoria: declarar veinte no mueve ni un byte el marco
    /// de pila. Es la prueba de que es un apodo y no un dato.
    #[test]
    fn los_88_no_ocupan_memoria() {
        let sin = compile_source_to_bef(&program("01 F PIC 9.", "MOVE 1 TO F.")).unwrap();
        let con = compile_source_to_bef(&program(
            "01 F PIC 9.\n88 A VALUE 1.\n88 B VALUE 2.\n88 C VALUE 3.",
            "MOVE 1 TO F.",
        ))
        .unwrap();
        assert_eq!(
            code_section(&sin).len(),
            code_section(&con).len(),
            "tres nombres de condicion no deben cambiar ni un byte del codigo"
        );
    }

    // ── OCCURS: lo que se RECHAZA, y diciendo qué hacer ─────────────────

    /// Un `OCCURS` en el nivel 01 no existe en el estándar. Se dice, y se
    /// enseña la forma buena: el grupo.
    #[test]
    fn occurs_en_nivel_01_se_rechaza_ensenando_el_grupo() {
        let src = program("01 E PIC 9(3) OCCURS 3 TIMES.", "MOVE 1 TO E(1).");
        let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
        assert!(t.contains("OCCURS en el nivel 01"), "{t}");
        assert!(t.contains("05 E PIC"), "el error tiene que ensenar el grupo: {t}");
    }

    /// Una tabla sin subíndice no es "el primer elemento": es una pregunta sin
    /// respuesta. Antes esto compilaba a un acceso al primero.
    #[test]
    fn una_tabla_sin_subindice_se_rechaza() {
        let src = program("01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.", "MOVE 1 TO E.");
        let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
        assert!(t.contains("es una tabla") && t.contains("E(I)"), "{t}");
    }

    /// Un subíndice literal que se sale NO compila. Es un error del programa,
    /// no una desgracia que descubrir de noche.
    #[test]
    fn un_subindice_literal_fuera_de_rango_no_compila() {
        let src = program("01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.", "MOVE 1 TO E(4).");
        let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
        assert!(t.contains("se sale") && t.contains("de 1 a 3"), "{t}");
    }

    /// `COMPUTE` con subíndice se rechaza porque su tokenizador lee el
    /// paréntesis como precedencia. Se dice, y se da la salida.
    #[test]
    fn compute_con_subindice_se_rechaza_dando_la_salida() {
        let src = program(
            "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.\n01 A PIC 9(3).",
            "COMPUTE A = E(1) + 1.",
        );
        let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
        assert!(t.contains("COMPUTE no admite subindices") && t.contains("MOVE"), "{t}");
    }

    /// Un dato que nadie declaró se rechaza. Antes `load_var`/`store_var` no
    /// emitían NADA: `DISPLAY PEPE` imprimía lo que hubiera en `rax` y
    /// `MOVE 1 TO PEPE` se perdía sin una palabra.
    #[test]
    fn un_dato_sin_declarar_se_rechaza() {
        let src = program("01 A PIC 9(3).", "MOVE 1 TO PEPE.");
        let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
        assert!(t.contains("PEPE") && t.contains("no esta declarado"), "{t}");
    }

    #[test]
    fn if_takes_only_the_true_branch() {
        let out = run_cobol(&program(
            "01 A PIC 9(3).\n01 B PIC 9(3).",
            "MOVE 7 TO A.\nMOVE 3 TO B.\n\
             IF A > B\n  DISPLAY \"MAYOR\"\nELSE\n  DISPLAY \"MENOR\"\nEND-IF.",
        ));
        assert_eq!(out, "MAYOR\n");
    }

    #[test]
    fn if_takes_only_the_else_branch() {
        let out = run_cobol(&program(
            "01 A PIC 9(3).\n01 B PIC 9(3).",
            "MOVE 2 TO A.\nMOVE 9 TO B.\n\
             IF A > B\n  DISPLAY \"MAYOR\"\nELSE\n  DISPLAY \"MENOR\"\nEND-IF.",
        ));
        assert_eq!(out, "MENOR\n");
    }

    /// Las condiciones en palabras del estándar deben decidir igual que los
    /// símbolos.
    #[test]
    fn worded_conditions_decide_the_same() {
        for (cond, expected) in [
            ("A IS EQUAL TO 5", "SI\n"),
            ("A IS GREATER THAN 5", "NO\n"),
            ("A IS NOT EQUAL TO 4", "SI\n"),
            ("A IS LESS THAN 6", "SI\n"),
            ("A IS NOT LESS THAN 5", "SI\n"),
        ] {
            let out = run_cobol(&program(
                "01 A PIC 9(3).",
                &format!("MOVE 5 TO A.\nIF {cond}\n  DISPLAY \"SI\"\nELSE\n  DISPLAY \"NO\"\nEND-IF."),
            ));
            assert_eq!(out, expected, "condicion: {cond}");
        }
    }

    /// Varias condiciones se conjugan con AND y cortocircuitan.
    #[test]
    fn and_conditions_need_all_of_them() {
        let out = run_cobol(&program(
            "01 A PIC 9(3).\n01 B PIC 9(3).",
            "MOVE 5 TO A.\nMOVE 1 TO B.\n\
             IF A > 3 AND B > 3\n  DISPLAY \"AMBAS\"\nELSE\n  DISPLAY \"NO\"\nEND-IF.",
        ));
        assert_eq!(out, "NO\n");
    }

    #[test]
    fn perform_times_repeats_exactly_n_times() {
        let out = run_cobol(&program("01 A PIC 9(3).", "PERFORM 3 TIMES\n  DISPLAY \"X\"\nEND-PERFORM."));
        assert_eq!(out, "X\nX\nX\n");
    }

    /// Cero iteraciones también es una respuesta: el contador se prueba
    /// ANTES de entrar.
    #[test]
    fn perform_zero_times_does_not_enter() {
        let out = run_cobol(&program("01 A PIC 9(3).", "PERFORM 0 TIMES\n  DISPLAY \"X\"\nEND-PERFORM."));
        assert_eq!(out, "");
    }

    /// `PERFORM UNTIL` con un contador real: prueba que el bucle avanza y
    /// que TERMINA (el emulador aborta si no).
    #[test]
    fn perform_until_loops_and_terminates() {
        let out = run_cobol(&program(
            "01 I PIC 9(3).",
            "MOVE 0 TO I.\nPERFORM UNTIL I >= 3\n  DISPLAY \"T\"\n  ADD 1 TO I\nEND-PERFORM.",
        ));
        assert_eq!(out, "T\nT\nT\n");
    }

    /// La aritmética tiene que aceptar VARIABLES, no solo literales: antes
    /// todo operando se parseaba como número y `ADD A TO T` sumaba cero.
    #[test]
    fn arithmetic_accepts_variables_as_operands() {
        let out = run_cobol(&program(
            "01 A PIC 9(3).\n01 T PIC 9(3).",
            "MOVE 5 TO A.\nMOVE 0 TO T.\nADD A TO T.\nADD A TO T.\n\
             IF T = 10\n  DISPLAY \"DIEZ\"\nELSE\n  DISPLAY \"MAL\"\nEND-IF.",
        ));
        assert_eq!(out, "DIEZ\n");
    }

    #[test]
    fn subtract_computes_dst_minus_src() {
        let out = run_cobol(&program(
            "01 A PIC 9(3).\n01 T PIC 9(3).",
            "MOVE 3 TO A.\nMOVE 10 TO T.\nSUBTRACT A FROM T.\n\
             IF T = 7\n  DISPLAY \"SIETE\"\nELSE\n  DISPLAY \"MAL\"\nEND-IF.",
        ));
        assert_eq!(out, "SIETE\n");
    }

    /// `COMPUTE` con precedencia real. Antes intentaba parsear la expresión
    /// entera como un número, fallaba, y guardaba 0 sin decir nada.
    #[test]
    fn compute_respects_precedence() {
        let out = run_cobol(&program(
            "01 T PIC 9(3).",
            "COMPUTE T = 2 + 3 * 4.\nIF T = 14\n  DISPLAY \"CATORCE\"\nELSE\n  DISPLAY \"MAL\"\nEND-IF.",
        ));
        assert_eq!(out, "CATORCE\n");
    }

    #[test]
    fn compute_respects_parentheses() {
        let out = run_cobol(&program(
            "01 T PIC 9(3).",
            "COMPUTE T = (2 + 3) * 4.\nIF T = 20\n  DISPLAY \"VEINTE\"\nELSE\n  DISPLAY \"MAL\"\nEND-IF.",
        ));
        assert_eq!(out, "VEINTE\n");
    }

    /// El alma bancaria: dinero en `PIC 9(3)V99` se opera en centavos, sin
    /// punto flotante. 10.05 + 0.20 = 10.25 EXACTO.
    #[test]
    fn money_arithmetic_stays_exact() {
        let out = run_cobol(&program(
            "01 SALDO PIC 9(3)V99.",
            "MOVE 10.05 TO SALDO.\nADD 0.20 TO SALDO.\n\
             IF SALDO = 10.25\n  DISPLAY \"EXACTO\"\nELSE\n  DISPLAY \"MAL\"\nEND-IF.",
        ));
        assert_eq!(out, "EXACTO\n");
    }

    /// Mezclar PICs de distinta escala exige reescalar; si no, se sumarían
    /// pesos con centavos.
    #[test]
    fn mixed_scales_rescale_before_operating() {
        let out = run_cobol(&program(
            "01 SALDO PIC 9(3)V99.\n01 ENTERO PIC 9(3).",
            "MOVE 2 TO ENTERO.\nMOVE 1.50 TO SALDO.\nADD ENTERO TO SALDO.\n\
             IF SALDO = 3.50\n  DISPLAY \"EXACTO\"\nELSE\n  DISPLAY \"MAL\"\nEND-IF.",
        ));
        assert_eq!(out, "EXACTO\n");
    }

    /// Un IF sin END-IF debe fallar con un mensaje claro, no compilar algo
    /// distinto de lo escrito.
    #[test]
    fn unterminated_if_is_an_error() {
        let src = program("01 A PIC 9(3).", "IF A > 1\n  DISPLAY \"X\"");
        let err = compile_source_to_bef(&src).unwrap_err();
        assert!(err.message.contains("END-IF"), "mensaje: {}", err.message);
    }

    // ── AND / OR: la condición dejó de ser una lista ────────────────────
    //
    // Era una `Vec` conjugada siempre con AND, y el `OR` se rechazaba con su
    // motivo. Ahora es un ÁRBOL, y lo que hay que probar no es que compile:
    // es que **decida bien**, incluida la precedencia y el cortocircuito.

    /// Las cuatro combinaciones de un `OR`, ejecutadas. Un emisor que colapsara
    /// el OR en un AND fallaría en las dos de en medio.
    #[test]
    fn el_or_decide_por_las_cuatro_esquinas() {
        let casos: &[(u32, u32, &str)] = &[
            (5, 5, "si\n"), // las dos ciertas
            (5, 0, "si\n"), // sólo la primera
            (0, 5, "si\n"), // sólo la segunda
            (0, 0, "no\n"), // ninguna
        ];
        for &(a, b, esperado) in casos {
            let src = program(
                "01 A PIC 9(3).\n01 B PIC 9(3).",
                &format!(
                    "MOVE {a} TO A.\nMOVE {b} TO B.\n\
                     IF A > 1 OR B > 1\nDISPLAY \"si\"\nELSE\nDISPLAY \"no\"\nEND-IF."
                ),
            );
            assert_eq!(run_cobol(&src), esperado, "A={a} B={b}");
        }
    }

    /// Y las del `AND`, que antes funcionaba pero por otro camino: ahora pasa
    /// por el mismo árbol y hay que volver a ganárselo.
    #[test]
    fn el_and_sigue_decidiendo_por_las_cuatro_esquinas() {
        let casos: &[(u32, u32, &str)] = &[
            (5, 5, "si\n"),
            (5, 0, "no\n"),
            (0, 5, "no\n"),
            (0, 0, "no\n"),
        ];
        for &(a, b, esperado) in casos {
            let src = program(
                "01 A PIC 9(3).\n01 B PIC 9(3).",
                &format!(
                    "MOVE {a} TO A.\nMOVE {b} TO B.\n\
                     IF A > 1 AND B > 1\nDISPLAY \"si\"\nELSE\nDISPLAY \"no\"\nEND-IF."
                ),
            );
            assert_eq!(run_cobol(&src), esperado, "A={a} B={b}");
        }
    }

    /// ★ LA PRECEDENCIA. `AND` liga más fuerte que `OR`, así que
    /// `A OR B AND C` es `A OR (B AND C)` y **no** `(A OR B) AND C`.
    ///
    /// Con `A` cierta y `C` falsa las dos lecturas discrepan: la buena dice sí
    /// (porque `A` sola basta), la mala dice no. Es exactamente el caso que un
    /// árbol mal montado compila sin quejarse y manda a la otra rama.
    #[test]
    fn and_liga_mas_fuerte_que_or() {
        let src = program(
            "01 A PIC 9(3).\n01 B PIC 9(3).\n01 C PIC 9(3).",
            "MOVE 5 TO A.\nMOVE 5 TO B.\nMOVE 0 TO C.\n\
             IF A > 1 OR B > 1 AND C > 1\nDISPLAY \"si\"\nELSE\nDISPLAY \"no\"\nEND-IF.",
        );
        assert_eq!(run_cobol(&src), "si\n", "se leyo (A OR B) AND C en vez de A OR (B AND C)");

        // Y la de al lado, para que no pase por casualidad: con A falsa, el
        // resultado tiene que venir del AND entero.
        let src = program(
            "01 A PIC 9(3).\n01 B PIC 9(3).\n01 C PIC 9(3).",
            "MOVE 0 TO A.\nMOVE 5 TO B.\nMOVE 0 TO C.\n\
             IF A > 1 OR B > 1 AND C > 1\nDISPLAY \"si\"\nELSE\nDISPLAY \"no\"\nEND-IF.",
        );
        assert_eq!(run_cobol(&src), "no\n");
    }

    /// Tres o más unidas, y mezcladas. Un fold que se dejara la última daría
    /// verde en los casos de dos y fallaría aquí.
    #[test]
    fn se_encadenan_mas_de_dos() {
        let src = program(
            "01 A PIC 9(3).\n01 B PIC 9(3).\n01 C PIC 9(3).",
            "MOVE 0 TO A.\nMOVE 0 TO B.\nMOVE 7 TO C.\n\
             IF A = 9 OR B = 9 OR C = 7\nDISPLAY \"si\"\nELSE\nDISPLAY \"no\"\nEND-IF.",
        );
        assert_eq!(run_cobol(&src), "si\n");

        let src = program(
            "01 A PIC 9(3).\n01 B PIC 9(3).\n01 C PIC 9(3).",
            "MOVE 1 TO A.\nMOVE 2 TO B.\nMOVE 3 TO C.\n\
             IF A = 1 AND B = 2 AND C = 3\nDISPLAY \"si\"\nELSE\nDISPLAY \"no\"\nEND-IF.",
        );
        assert_eq!(run_cobol(&src), "si\n");
    }

    /// ★ El `OR` dentro de un `PERFORM UNTIL` — que es donde vive de verdad en
    /// un batch: *"hasta que se acabe el fichero **o** hasta que algo vaya
    /// mal"*. Sin él, un proceso nocturno no puede pararse por error.
    #[test]
    fn un_perform_until_para_con_cualquiera_de_las_dos() {
        let src = program(
            "01 I PIC 9(3).\n01 ERROR-SW PIC 9.",
            "MOVE 0 TO I.\nMOVE 0 TO ERROR-SW.\n\
             PERFORM UNTIL I = 10 OR ERROR-SW = 1\n\
             ADD 1 TO I\n\
             IF I = 4\nMOVE 1 TO ERROR-SW\nEND-IF\n\
             END-PERFORM.\nDISPLAY I.",
        );
        assert_eq!(run_cobol(&src), "4\n", "el bucle no paro por la segunda condicion");
    }

    /// ★ EL CORTOCIRCUITO, y no como optimización: si la primera falla, la
    /// segunda **no se evalúa**. Aquí se ve porque la segunda lleva un
    /// subíndice fuera de rango, y evaluarla mataría el programa con
    /// `SUBINDICE FUERA DE RANGO`.
    ///
    /// Es el patrón que un programa de banca escribe todo el rato: comprobar
    /// que el índice vale ANTES de usarlo.
    #[test]
    fn el_and_corta_antes_de_evaluar_la_segunda() {
        let src = program(
            "01 TABLA.\n05 T PIC 9(3) OCCURS 3 TIMES.\n01 I PIC 9(3).",
            "MOVE 9 TO I.\n\
             IF I <= 3 AND T(I) > 0\nDISPLAY \"dentro\"\nELSE\nDISPLAY \"fuera\"\nEND-IF.",
        );
        assert_eq!(run_cobol(&src), "fuera\n", "se evaluo T(9) y no debia");
    }

    /// El mismo corte por el otro lado: si la primera de un `OR` acierta, la
    /// segunda no se mira.
    #[test]
    fn el_or_corta_cuando_la_primera_acierta() {
        let src = program(
            "01 TABLA.\n05 T PIC 9(3) OCCURS 3 TIMES.\n01 I PIC 9(3).",
            "MOVE 9 TO I.\n\
             IF I > 3 OR T(I) > 0\nDISPLAY \"si\"\nELSE\nDISPLAY \"no\"\nEND-IF.",
        );
        assert_eq!(run_cobol(&src), "si\n", "se evaluo T(9) y no debia");
    }

    /// Un `OR` dentro de una comparación en palabras no es un `OR` lógico:
    /// `IS GREATER THAN OR EQUAL TO` lleva uno dentro. Partir por `OR` antes de
    /// normalizar cortaría la comparación por la mitad.
    #[test]
    fn el_or_de_greater_than_or_equal_no_es_un_or() {
        let src = program(
            "01 A PIC 9(3).",
            "MOVE 5 TO A.\nIF A IS GREATER THAN OR EQUAL TO 5\nDISPLAY \"si\"\nELSE\nDISPLAY \"no\"\nEND-IF.",
        );
        assert_eq!(run_cobol(&src), "si\n");
    }

    /// El ejemplo del repositorio, ejecutado. Si alguien vuelve a romper el
    /// flujo de control, este test lo dice antes de que haga falta flashear
    /// nada.
    #[test]
    fn banco_example_produces_its_documented_output() {
        let out = run_cobol(include_str!("../examples/2-decimal/banco.cob"));
        assert_eq!(
            out,
            // ★ `59.97` y `19.99` NO son literales del programa: son el
            // contenido de SALDO formateado en ejecución por el código que
            // emite `emit_display_var`. Antes el ejemplo imprimía una cadena
            // escrita a mano que decía el resultado — la aritmética era real
            // pero lo que se veía no lo demostraba. Ahora sí: si el decimal se
            // perdiera, este test lo cazaría solo.
            "BMO-X: caja COBOL\n\
             cobrada una cuota\ncobrada una cuota\ncobrada una cuota\n\
             saldo tras 3 cuotas:\n59.97\ncuadra\n\
             recibo emitido\nrecibo emitido\n\
             saldo tras 2 devoluciones:\n19.99\n\
             dos devoluciones aplicadas\n"
        );
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

    /// La E/S de ficheros, tal y como se escribe de verdad: el `SELECT` le da
    /// la ruta, el `FD` le da el registro y el `READ` lleva su `AT END`.
    ///
    /// Este test decía antes `READ INFILE INTO WS-REC.` sin `SELECT`, sin `FD`
    /// y sin `AT END`, y pasaba — porque el parser guardaba dos cadenas y el
    /// codegen las tiraba. Ahora un fichero es un fichero: si le falta la ruta
    /// o el registro, no compila.
    #[test]
    fn parses_open_read_write_close() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. FILEIO.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT INFILE ASSIGN TO "datos/mov.txt".
DATA DIVISION.
FILE SECTION.
FD INFILE.
01 WS-REC PIC 9(5).
PROCEDURE DIVISION.
OPEN INPUT INFILE.
READ INFILE
    AT END DISPLAY "fin"
    NOT AT END DISPLAY WS-REC
END-READ.
WRITE WS-REC.
CLOSE INFILE.
STOP RUN.
"#;
        let program = parse(src).unwrap();
        assert_eq!(program.statements.len(), 5);
        // La ruta y el registro llegan al AST: sin los dos no hay E/S.
        let f = program.file("INFILE").expect("el SELECT declara INFILE");
        assert_eq!(f.path, "datos/mov.txt");
        assert_eq!(f.record, "WS-REC");
        // Y el READ se queda con sus DOS ramas, no con una cadena.
        match &program.statements[1] {
            CobolStatement::Read(nombre, al_final, si_hay) => {
                assert_eq!(nombre, "INFILE");
                assert_eq!(al_final.len(), 1);
                assert_eq!(si_hay.len(), 1);
            }
            otro => panic!("se esperaba un READ, no {otro:?}"),
        }
    }

    /// `PERFORM` ahora exige cuerpo y cierre: sin ellos no había nada que
    /// repetir, y la versión anterior lo aceptaba emitiendo un no-op.
    #[test]
    fn parses_perform() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. LOOP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-COUNT PIC 9(3).
PROCEDURE DIVISION.
PERFORM 5 TIMES
  ADD 1 TO WS-COUNT
END-PERFORM.
PERFORM UNTIL WS-COUNT > 10
  ADD 1 TO WS-COUNT
END-PERFORM.
STOP RUN.
"#;
        let program = parse(src).unwrap();
        assert!(program.statements.len() >= 2);
        assert!(matches!(program.statements[0], CobolStatement::PerformTimes(5, _)));
        assert!(matches!(program.statements[1], CobolStatement::PerformUntil(_, _)));
    }

    /// El bucle anterior, ejecutado: 5 sumas y luego hasta pasar de 10.
    #[test]
    fn nested_loops_reach_the_expected_total() {
        let out = run_cobol(&program(
            "01 WS-COUNT PIC 9(3).",
            "MOVE 0 TO WS-COUNT.\n\
             PERFORM 5 TIMES\n  ADD 1 TO WS-COUNT\nEND-PERFORM.\n\
             PERFORM UNTIL WS-COUNT > 10\n  ADD 1 TO WS-COUNT\nEND-PERFORM.\n\
             IF WS-COUNT = 11\n  DISPLAY \"ONCE\"\nELSE\n  DISPLAY \"MAL\"\nEND-IF.",
        ));
        assert_eq!(out, "ONCE\n");
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
