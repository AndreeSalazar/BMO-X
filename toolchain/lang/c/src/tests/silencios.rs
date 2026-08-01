//! Los ceros silenciosos: identificadores y llamadas que no existen
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// ═══════════════ Los tres silencios que escondían todo esto ═══════════════

/// ★ `#include` tiraba los `#define` de la cabecera.
///
/// Y no fallaba: la directiva se consumía, el identificador seguía en el
/// texto y el codegen lo ponía a cero. Dos constantes distintas se volvían
/// la MISMA variable inventada, así que compararlas era cierto.
#[test]
fn una_cabecera_incluida_deja_sus_constantes() {
    let out = run_c_con_pp(
        "#include <bmo/entrada.h>
         int main() { printf(\"%d %d\\n\", BMO_TECLA_REPAG, BMO_TECLA_AVPAG); return 0; }",
    );
    assert_eq!(out.trim(), "135 136", "REPAG y AVPAG no pueden valer lo mismo");
}

/// La misma cabecera dos veces no duplica lo que trae. El guardia
/// `#ifndef` sólo puede funcionar si el `#define` del guardia sobrevive al
/// `#include` — antes no sobrevivía, así que el guardia no guardaba nada.
#[test]
fn incluir_dos_veces_no_duplica_la_cabecera() {
    let out = run_c_con_pp(
        "#include <bmo/scroll.h>
#include <bmo/entrada.h>
#include <bmo/bmo.h>
         int main() { printf(\"%d\\n\", bmo_scroll_mover(0, 4, 200, 16)); return 0; }",
    );
    assert_eq!(out.trim(), "4");
}

/// Un nombre que no existe NO VALE CERO. Un cero inventado es la peor
/// respuesta posible: es legítimo en cualquier expresión, así que el error
/// viaja hasta donde ya no se puede rastrear.
#[test]
fn un_identificador_que_no_existe_es_un_error_no_un_cero() {
    let err = compile_source_to_bef("int main() { return NO_EXISTE; }")
        .expect_err("un nombre sin declarar tiene que fallar");
    assert!(err.message.contains("NO_EXISTE"), "mensaje: {}", err.message);
}

/// Y una llamada sin destino tampoco es un hueco: `E8 00000000` es "llama a
/// la siguiente instrucción", o sea un no-op con dirección de retorno.
#[test]
fn llamar_a_una_funcion_que_no_existe_es_un_error() {
    let err = compile_source_to_bef("int main() { fantasma(1); return 0; }")
        .expect_err("llamar a lo que no existe tiene que fallar");
    assert!(err.message.contains("fantasma"), "mensaje: {}", err.message);
}
