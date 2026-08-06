//! ARITMETICA — 12 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

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

