//! TABLAS -- 4 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

// -- OCCURS: lo que se RECHAZA, y diciendo que hacer -----------------

/// Un `OCCURS` en el nivel 01 no existe en el estandar. Se dice, y se
/// ensena la forma buena: el grupo.
#[test]
fn occurs_en_nivel_01_se_rechaza_ensenando_el_grupo() {
    let src = program("01 E PIC 9(3) OCCURS 3 TIMES.", "MOVE 1 TO E(1).");
    let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
    assert!(t.contains("OCCURS en el nivel 01"), "{t}");
    assert!(t.contains("05 E PIC"), "el error tiene que ensenar el grupo: {t}");
}

/// Una tabla sin subindice no es "el primer elemento": es una pregunta sin
/// respuesta. Antes esto compilaba a un acceso al primero.
#[test]
fn una_tabla_sin_subindice_se_rechaza() {
    let src = program("01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.", "MOVE 1 TO E.");
    let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
    assert!(t.contains("es una tabla") && t.contains("E(I)"), "{t}");
}

/// Un subindice literal que se sale NO compila. Es un error del programa,
/// no una desgracia que descubrir de noche.
#[test]
fn un_subindice_literal_fuera_de_rango_no_compila() {
    let src = program("01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.", "MOVE 1 TO E(4).");
    let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
    assert!(t.contains("se sale") && t.contains("de 1 a 3"), "{t}");
}

/// `COMPUTE` con subindice se rechaza porque su tokenizador lee el
/// parentesis como precedencia. Se dice, y se da la salida.
#[test]
fn compute_con_subindice_se_rechaza_dando_la_salida() {
    let src = program(
        "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.\n01 A PIC 9(3).",
        "COMPUTE A = E(1) + 1.",
    );
    let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
    assert!(t.contains("COMPUTE no admite subindices") && t.contains("MOVE"), "{t}");
}

