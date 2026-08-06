//! DESBORDES — 7 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

// ── ON SIZE ERROR: qué pasa cuando el resultado NO CABE ─────────────
//
// Sin la cláusula, COBOL guarda el número recortado por arriba y sigue. Con
// ella, el campo **no se toca** y el programa decide.

/// ★ LA PARTE QUE IMPORTA: cuando no cabe, **el destino se queda como
/// estaba**. No es un tecnicismo — deja el saldo anterior intacto para que
/// el programa lo pueda escribir en un informe de rechazos y seguir.
#[test]
fn on_size_error_no_toca_el_campo() {
    let src = program(
        "01 A PIC 9(3) VALUE 123.",
        "ADD 900 TO A ON SIZE ERROR\nDISPLAY \"no cabe\"\nEND-ADD.\nDISPLAY A.",
    );
    // 123 + 900 = 1023, y en tres dígitos no entra.
    assert_eq!(run_cobol(&src), "no cabe\n123\n", "el campo se toco igualmente");
}

/// ⚠ Y sin la cláusula, **BMO se queda con el número entero**: `1023` en un
/// `PIC 9(3)`.
///
/// Eso **no es lo que dice el estándar** —COBOL recorta por arriba y
/// guardaría `023`— y es una divergencia conocida: un campo `DISPLAY` de
/// BMO sigue siendo un entero de 64 bits y no mide lo que dice su PICTURE.
/// Es la tarea `1.5` del plan, la única de la fase 1 que sigue abierta.
///
/// Se fija aquí a propósito. El día que `1.5` entre, este test **tiene que
/// cambiar**, y ése es justo el aviso que hace falta: un cambio de
/// almacenamiento que altera resultados no puede pasar callando.
///
/// Mientras tanto tiene una consecuencia buena: hoy `ON SIZE ERROR` es lo
/// ÚNICO que caza un desbordamiento en BMO.
#[test]
fn sin_on_size_error_bmo_no_recorta_todavia() {
    let src = program("01 A PIC 9(3) VALUE 123.", "ADD 900 TO A.\nDISPLAY A.");
    assert_eq!(
        run_cobol(&src),
        "1023\n",
        "si esto da 023, es que 1.5 entro y hay que actualizar el plan"
    );
}

/// `NOT ON SIZE ERROR` — lo que se hace cuando SÍ cupo.
#[test]
fn not_on_size_error_corre_cuando_cabe() {
    let src = program(
        "01 A PIC 9(5) VALUE 123.\n01 N PIC 9(3) VALUE ZERO.",
        "ADD 900 TO A ON SIZE ERROR\nDISPLAY \"no cabe\"\n\
         NOT ON SIZE ERROR\nADD 1 TO N\nEND-ADD.\nDISPLAY A.\nDISPLAY N.",
    );
    assert_eq!(run_cobol(&src), "1023\n1\n");
}

/// ★ DIVIDIR ENTRE CERO es un desborde, no un fallo del CPU.
///
/// Sin esto, el `idiv` levanta `#DE` y el proceso muere sin decir por qué.
/// En un batch eso es peor que un número malo: se lleva por delante el
/// proceso entero por culpa de un registro.
#[test]
fn dividir_entre_cero_es_un_desborde_y_no_una_muerte() {
    let src = program(
        "01 A PIC S9(7)V99 VALUE 100.00.\n01 D PIC 9(3) VALUE ZERO.",
        "DIVIDE D BY A ON SIZE ERROR\nDISPLAY \"division por cero\"\nEND-DIVIDE.\n\
         DISPLAY A.",
    );
    assert_eq!(run_cobol(&src), "division por cero\n100.00\n");
}

/// La cláusula vale en las cinco, no sólo en `ADD`.
#[test]
fn on_size_error_vale_en_las_cinco() {
    let casos: &[(&str, &str)] = &[
        ("01 A PIC 9(3) VALUE 999.", "ADD 999 TO A ON SIZE ERROR\nDISPLAY \"x\"\nEND-ADD."),
        ("01 A PIC 9(3) VALUE 100.", "SUBTRACT 9999 FROM A ON SIZE ERROR\nDISPLAY \"x\"\nEND-SUBTRACT."),
        ("01 A PIC 9(3) VALUE 999.", "MULTIPLY 999 BY A ON SIZE ERROR\nDISPLAY \"x\"\nEND-MULTIPLY."),
        ("01 A PIC 9(3) VALUE 100.", "DIVIDE 0 BY A ON SIZE ERROR\nDISPLAY \"x\"\nEND-DIVIDE."),
        ("01 A PIC 9(3) VALUE 1.", "COMPUTE A = 999 * 999 ON SIZE ERROR\nDISPLAY \"x\"\nEND-COMPUTE."),
    ];
    for (data, body) in casos {
        let src = program(data, body);
        assert_eq!(run_cobol(&src), "x\n", "no salto el desborde en: {body}");
    }
}

/// Un `SUBTRACT` que se pasa por abajo también desborda: `-9899` no cabe en
/// un `PIC 9(3)`, y el signo no cambia la cuenta de dígitos.
#[test]
fn el_desborde_mira_la_magnitud_no_el_signo() {
    let src = program(
        "01 A PIC S9(3) VALUE 100.",
        "SUBTRACT 9999 FROM A ON SIZE ERROR\nDISPLAY \"no cabe\"\nEND-SUBTRACT.\nDISPLAY A.",
    );
    assert_eq!(run_cobol(&src), "no cabe\n100\n");
}

/// Sin `END-<verbo>` no se sabe dónde acaba la cláusula, y tragarse lo de
/// después la convertiría en el resto del programa.
#[test]
fn una_clausula_sin_cierre_se_rechaza() {
    let src = program("01 A PIC 9(3).", "ADD 1 TO A ON SIZE ERROR\nDISPLAY \"x\".");
    let err = compile_source_to_bef(&src).unwrap_err().to_string();
    assert!(err.contains("END-ADD"), "{err}");
}

