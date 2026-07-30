pub mod ast;
pub mod codegen;
pub mod dialect;
pub mod edicion;
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
            &[("apps/movim.txt", "1000.00\n234.56\n0.44\n-100.00\n")],
        );
        let esperado = [
            "BATCH DE CIERRE - BANCO BMO",
            "total del dia:",
            " $1,135.00",
            "cierre escrito en apps/cierre.txt",
        ]
        .map(|l| format!("{l}\n"))
        .concat();
        assert_eq!(salida, esperado);
        // Y en el disco queda el total, no un fichero vacío ni a medias.
        assert_eq!(m.archivo_texto("apps/cierre.txt").as_deref(), Some("1135.00\n"));
    }

    /// Un fichero que no existe NO es un fichero vacío: el `AT END` salta a la
    /// primera y el total es cero, sin reventar. En un batch nocturno eso es
    /// la diferencia entre "hoy no hubo movimientos" y una caída.
    #[test]
    fn un_fichero_que_falta_da_cero_y_no_revienta() {
        let (salida, m) = run_cobol_con_disco(include_str!("../examples/4-ficheros/batch.cob"), &[]);
        assert!(salida.contains("total del dia:"), "{salida}");
        assert!(salida.contains("     $0.00"), "{salida}");
        assert_eq!(m.archivo_texto("apps/cierre.txt").as_deref(), Some("0.00\n"));
    }

    /// El último registro cuenta aunque el fichero no acabe en salto de línea.
    /// Es el clásico que se come el movimiento de más valor: el último.
    #[test]
    fn el_ultimo_registro_cuenta_sin_salto_final() {
        let (salida, _) = run_cobol_con_disco(
            include_str!("../examples/4-ficheros/batch.cob"),
            &[("apps/movim.txt", "10.00\n5.50")],
        );
        assert!(salida.contains("    $15.50"), "{salida}");
    }

    /// Los ficheros escritos desde el anfitrión traen `\r\n`. Ese `\r` dentro
    /// del número lo convertiría en otro.
    #[test]
    fn el_batch_aguanta_los_finales_de_windows() {
        let (salida, _) = run_cobol_con_disco(
            include_str!("../examples/4-ficheros/batch.cob"),
            &[("apps/movim.txt", "1000.00\r\n234.56\r\n")],
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
                ("apps/concs.txt", "1\n3\n2\n3\n1\n"),
                ("apps/imps.txt", "100.00\n50.00\n25.50\n10.00\n5.00\n"),
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
            &[("apps/concs.txt", "1\n7\n"), ("apps/imps.txt", "100.00\n50.00\n")],
        );
        assert!(
            salida.contains("SUBINDICE FUERA DE RANGO EN TOTAL-CONCEPTO (1..4)"),
            "{salida}"
        );
        assert!(!salida.contains("totales por concepto"), "no debe seguir: {salida}");
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

    /// `OR` se rechaza en vez de compilarse como si fuera `AND`.
    #[test]
    fn or_conditions_are_rejected_not_miscompiled() {
        let src = program(
            "01 A PIC 9(3).",
            "IF A > 1 OR A < 0\n  DISPLAY \"X\"\nEND-IF.",
        );
        let err = compile_source_to_bef(&src).unwrap_err();
        assert!(err.message.contains("OR"), "mensaje: {}", err.message);
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
