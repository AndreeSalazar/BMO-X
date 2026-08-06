//! GRUPOS — 6 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

// ── EL ÁREA DE REGISTRO: grupos con los campos en su byte ───────────
//
// Camino B de PLAN_BANCA §1.0. Un grupo tiene dos representaciones: las
// ranuras de trabajo (un entero escalado por campo) y el ÁREA (los bytes
// tal cual irían al disco). `MOVE` de grupo pasa por el área.

/// ★ LA PRUEBA QUE NO SE PUEDE FINGIR: un `MOVE` de grupo pasa por los
/// BYTES, así que sobrevive a que los dos grupos tengan **nombres
/// distintos** en los campos — sólo tienen que coincidir en la forma.
///
/// Un emisor que copiara campo a campo por nombre fallaría aquí, y uno que
/// copiara por posición de declaración pasaría este test pero fallaría el
/// de abajo.
#[test]
fn un_move_de_grupo_copia_los_bytes_no_los_nombres() {
    let src = program(
        "01 ORIGEN.\n\
         05 O-A PIC 9(4).\n\
         05 O-B PIC S9(5)V99 COMP-3.\n\
         01 DESTINO.\n\
         05 D-X PIC 9(4).\n\
         05 D-Y PIC S9(5)V99 COMP-3.",
        "MOVE 1234 TO O-A.\nMOVE -99.95 TO O-B.\n\
         MOVE ORIGEN TO DESTINO.\n\
         DISPLAY D-X.\nDISPLAY D-Y.",
    );
    assert_eq!(run_cobol(&src), "1234\n-99.95\n");
}

/// ★ Y ÉSTA es la que prueba que el área son BYTES DE VERDAD y no un
/// atajo: los dos grupos tienen la **misma forma en bytes** pero **cortada
/// distinta**. Origen: un campo de 6 dígitos. Destino: dos de 3.
///
/// Copiar campo a campo no puede dar esto. Sólo sale bien si lo que viaja
/// son los bytes zonados — `123456` escrito como seis caracteres, y el
/// destino leyendo `123` y `456` de su sitio.
///
/// Es exactamente lo que un programa de banca hace para reinterpretar un
/// registro, y la razón por la que el estándar dice que un `MOVE` de grupo
/// no mira qué hay dentro.
#[test]
fn el_area_son_bytes_de_verdad_y_se_puede_recortar_distinto() {
    let src = program(
        "01 ORIGEN.\n\
         05 O-TODO PIC 9(6).\n\
         01 DESTINO.\n\
         05 D-ALTO PIC 9(3).\n\
         05 D-BAJO PIC 9(3).",
        "MOVE 123456 TO O-TODO.\n\
         MOVE ORIGEN TO DESTINO.\n\
         DISPLAY D-ALTO.\nDISPLAY D-BAJO.",
    );
    assert_eq!(
        run_cobol(&src),
        "123\n456\n",
        "el area no son bytes: el MOVE de grupo copio campo a campo"
    );
}

/// El signo sobrevive al viaje por el área, que es donde vive
/// sobrepunzado en el último dígito.
#[test]
fn el_signo_sobrevive_al_area() {
    let src = program(
        "01 ORIGEN.\n05 O-A PIC S9(5).\n\
         01 DESTINO.\n05 D-A PIC S9(5).",
        "MOVE -1234 TO O-A.\nMOVE ORIGEN TO DESTINO.\nDISPLAY D-A.",
    );
    assert_eq!(run_cobol(&src), "-1234\n");
}

/// Un grupo dentro de otro: los offsets se acumulan y el `MOVE` de arriba
/// se lleva todo lo de abajo.
#[test]
fn un_move_de_grupo_arrastra_los_grupos_de_dentro() {
    let src = program(
        "01 ORIGEN.\n\
         05 O-CAB.\n\
         10 O-TIPO PIC 9.\n\
         10 O-NUM PIC 9(4).\n\
         05 O-IMP PIC S9(5)V99 COMP-3.\n\
         01 DESTINO.\n\
         05 D-CAB.\n\
         10 D-TIPO PIC 9.\n\
         10 D-NUM PIC 9(4).\n\
         05 D-IMP PIC S9(5)V99 COMP-3.",
        "MOVE 7 TO O-TIPO.\nMOVE 4471 TO O-NUM.\nMOVE 1234.56 TO O-IMP.\n\
         MOVE ORIGEN TO DESTINO.\n\
         DISPLAY D-TIPO.\nDISPLAY D-NUM.\nDISPLAY D-IMP.",
    );
    assert_eq!(run_cobol(&src), "7\n4471\n1234.56\n");
}

/// Dos campos con el mismo nombre no se pueden distinguir en un `MOVE`.
/// COBOL lo resuelve con `A OF REG`, que todavía no existe — así que se
/// dice en vez de quedarse con uno de los dos en silencio.
#[test]
fn dos_campos_con_el_mismo_nombre_se_rechazan() {
    let src = program(
        "01 UNO.\n05 IMPORTE PIC 9(4).\n01 DOS.\n05 IMPORTE PIC 9(4).",
        "DISPLAY \"x\".",
    );
    let err = compile_source_to_bef(&src).unwrap_err().to_string();
    assert!(err.contains("dos veces"), "{err}");
}

/// Mezclar un grupo con un campo pide relleno con espacios, y eso necesita
/// que exista el texto. Se dice en vez de mover el primer campo y callar.
#[test]
fn mover_un_grupo_a_un_campo_se_rechaza() {
    let src = program(
        "01 G.\n05 A PIC 9(4).\n01 SUELTO PIC 9(4).",
        "MOVE G TO SUELTO.",
    );
    let err = compile_source_to_bef(&src).unwrap_err().to_string();
    assert!(err.contains("uno es un GRUPO"), "{err}");
}

