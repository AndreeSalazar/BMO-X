//! **El cotejo de disposicion, y sobre todo: que SABE DECIR QUE NO.**
//!
//! # De donde sale
//!
//! `codegen::build_struct_layout` recalcula la disposicion en vez de recibirla
//! del parser, y su cabecera lo justifica: es lo que impide que un frontend
//! distinto imponga la suya *"sin que se note"*.
//!
//! ** El argumento era bueno y la implementacion no lo cumplia: se calculaban
//! DOS disposiciones y no se comparaba ninguna. Dos cuentas que nadie contrasta
//! no son una comprobacion doble -- son dos oportunidades de equivocarse. Y ya
//! habia pasado: el 2026-08-13 divergieron y lo destapo un bug, no un guardian.
//!
//! # Por que este fichero existe y no basta con que las 454 pasen
//!
//! Que un guardian no se queje puede querer decir dos cosas muy distintas:
//! que todo cuadra, o que **no mira**. Las casillas de abajo separan las dos,
//! y son la misma exigencia que el contrato le hace a sus diecisiete reglas:
//! *saber decir que NO*.

use super::*;
use crate::ast::*;

/// Un programa minimo con UN struct, parseado de verdad.
fn programa_con_struct() -> Program {
    crate::parse("struct P { int a; int b; }; int main() { return 0; }")
        .expect("el programa de prueba tiene que parsear")
}

/// ** El caso normal: el frontend y el codegen dicen lo mismo, y compila.
///
/// [!] Esta casilla es la que da sentido a las otras dos. Sin ella, un cotejo
/// que rechazara SIEMPRE tambien pasaria por guardian.
#[test]
fn cuando_las_dos_disposiciones_coinciden_compila() {
    let p = programa_con_struct();
    assert!(
        !p.disposiciones.is_empty(),
        "el parser tiene que declarar lo que coloco, o no hay nada que cotejar"
    );
    assert!(crate::codegen::compile_to_bef_bytes(&p).is_ok());
}

/// ** UN OFFSET MOVIDO UN BYTE, y tiene que doler.
///
/// Es la forma exacta del fallo del 13-08: el mismo campo colocado en dos
/// sitios por dos cuentas distintas. Antes de este cotejo, el `.bex` salia y el
/// programa leia el campo de al lado.
#[test]
fn un_offset_que_no_cuadra_se_rechaza() {
    let mut p = programa_con_struct();
    let d = p.disposiciones.get_mut("P").expect("struct P colocado");
    d.campos[1].1 += 1; // `b` un byte mas alla de donde el codegen lo pone

    let e = crate::codegen::compile_to_bef_bytes(&p)
        .expect_err("una disposicion que no cuadra no puede compilar");
    assert!(
        e.message.contains("disposicion de `P`") && e.message.contains('b'),
        "el mensaje tiene que nombrar el agregado Y el campo: {}",
        e.message
    );
}

/// ** Y EL TAMANO TOTAL, que es la otra mitad.
///
/// Un tamano equivocado no mueve ningun campo de este struct: mueve al
/// SIGUIENTE elemento de cualquier array que lo contenga. Por eso se juzga
/// aparte de los offsets.
#[test]
fn un_tamano_que_no_cuadra_se_rechaza() {
    let mut p = programa_con_struct();
    p.disposiciones.get_mut("P").expect("struct P colocado").tamano += 8;

    let e = crate::codegen::compile_to_bef_bytes(&p)
        .expect_err("un tamano que no cuadra no puede compilar");
    assert!(
        e.message.contains("tamano"),
        "el mensaje tiene que decir que lo que no cuadra es el tamano: {}",
        e.message
    );
}

/// [!] **Un `disposiciones` vacio NO es un fallo**, y esto lo fija.
///
/// Significa *"este frontend no declara la suya"*, y entonces manda la del
/// codegen. Sin esta casilla, alguien endureceria el cotejo a "exige siempre"
/// y romperia a cualquier frontend que todavia no lo llene.
#[test]
fn un_frontend_que_no_declara_disposicion_sigue_compilando() {
    let mut p = programa_con_struct();
    p.disposiciones.clear();
    assert!(crate::codegen::compile_to_bef_bytes(&p).is_ok());
}
