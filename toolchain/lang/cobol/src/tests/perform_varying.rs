//! PERFORM VARYING -- 8 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

// -- PERFORM VARYING: el bucle CON INDICE ----------------------------

/// Lo minimo, y con el indice usable dentro del cuerpo.
#[test]
fn perform_varying_recorre_con_indice() {
    let src = program(
        "01 I PIC 9(3).\n01 SUMA PIC 9(5) VALUE ZERO.",
        "PERFORM VARYING I FROM 1 BY 1 UNTIL I > 5\n\
         ADD I TO SUMA\n\
         END-PERFORM.\n\
         DISPLAY SUMA.\nDISPLAY I.",
    );
    // 1+2+3+4+5 = 15, y al salir I vale 6 -- la vuelta que hizo fallar la
    // condicion tambien incremento.
    assert_eq!(run_cobol(&src), "15\n6\n");
}

/// [!] `UNTIL` dice cuando **PARAR**, no cuando seguir. Es al reves que el
/// `while` de casi todo lo demas, y confundirlo da una vuelta de mas o de
/// menos -- que sobre una tabla es un subindice fuera de rango.
#[test]
fn el_until_dice_cuando_parar() {
    // `UNTIL I > 3` recorre 1,2,3 -- no llega al 4.
    let src = program(
        "01 I PIC 9(3).\n01 T PIC X(8) VALUE SPACES.",
        "PERFORM VARYING I FROM 1 BY 1 UNTIL I > 3\n\
         DISPLAY I\n\
         END-PERFORM.",
    );
    assert_eq!(run_cobol(&src), "1\n2\n3\n");
}

/// `WITH TEST BEFORE`: si la condicion ya se cumple al entrar, el cuerpo
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

/// El paso puede ser distinto de uno, y **hacia atras**.
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

/// * `AFTER` -- y lo que de verdad prueba: el de dentro **se reinicia** cada
/// vez que el de fuera avanza.
///
/// Sin ese reinicio la tabla se recorre en diagonal: la primera fila entera
/// y de las demas solo la ultima columna. Por eso el test cuenta las
/// vueltas: tienen que ser 3 x 4, no 3 + 4.
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
    assert_eq!(run_cobol(&src), "30\n"); // 2 x 3 x 5
}

/// * EL CASO POR EL QUE EXISTE: recorrer una tabla con `OCCURS`.
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

