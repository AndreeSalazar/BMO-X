//! TEXTO COMPUESTO — 6 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

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

