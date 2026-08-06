//! CONDICIONES — 14 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

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

