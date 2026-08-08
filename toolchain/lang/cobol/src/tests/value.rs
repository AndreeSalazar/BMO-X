//! VALUE -- 7 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

// -- VALUE: el valor con el que arranca un dato ----------------------
//
// Se parseaba desde siempre y no se emitia nunca. Un campo declarado con
// VALUE arrancaba con lo que hubiera en la pila, y ningun ejemplo lo
// destapaba porque todos inicializan a mano con MOVE.

/// Sin un solo `MOVE`: el dato ya vale lo que dice su declaracion.
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

/// El estandar dice que un `VALUE` sobre una tabla llena **todas** las
/// casillas, no la primera.
#[test]
fn value_sobre_una_tabla_llena_todas_las_casillas() {
    let src = program(
        "01 TABLA.\n05 T PIC S9(5)V99 VALUE 9.99 OCCURS 3 TIMES.",
        "DISPLAY T(1).\nDISPLAY T(2).\nDISPLAY T(3).",
    );
    assert_eq!(run_cobol(&src), "9.99\n9.99\n9.99\n");
}

/// El `VALUE` se pone ANTES de la primera sentencia, asi que un `MOVE`
/// posterior manda. Al reves --inicializar al final-- borraria lo que el
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
        // `VALUE "HOLA"` sobre un `PIC X` ya NO esta aqui: desde que existe
        // el texto (0.7), se guarda como caracteres. Sobre un campo
        // NUMERICO sigue sin tener sentido, y eso es lo que queda.
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

/// Sin `VALUE` no compara nada. Antes de existir el 88, esto habria sido un
/// dato sin PIC con un nombre suelto.
#[test]
fn un_88_sin_value_se_rechaza() {
    let src = program("01 F PIC 9.\n88 FIN.", "MOVE 1 TO F.");
    let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
    assert!(t.contains("necesita su VALUE"), "{t}");
}

