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
        let total = cases.len();
        assert!(broken.is_empty(), "\n{}/{} FUNCIONAN. ROTOS:\n{}", total - broken.len(), total, broken.join("\n"));
    }


    /// El payload `hola_COBOL.bex` que el kernel EMBEBE, ejecutado.
    ///
    /// Regenerar tras tocar el codegen:
    ///   cargo run -p bmo-cobol-front --     ///     toolchain/lang/cobol/examples/hola_COBOL.cob     ///     -o Ultra_kernel_x86-64/kernel/src/ring0/hola_COBOL.bex
    #[test]
    fn hola_cobol_payload_output_is_what_the_kernel_will_show() {
        let out = run_cobol(include_str!("../examples/hola_COBOL.cob"));
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
        let out = run_cobol(include_str!("../examples/banco.cob"));
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
