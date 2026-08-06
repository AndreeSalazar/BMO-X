//! ROUNDED — 7 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

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

