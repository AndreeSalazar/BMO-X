//! EVALUATE — 8 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

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

