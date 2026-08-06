//! TEXTO — 7 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

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

