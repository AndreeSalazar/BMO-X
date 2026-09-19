//! **`WANTS_SCREEN`: la bandera que pone el COMPILADOR, y cuando NO la pone.**
//!
//! La deduce `codegen` al ver `BMO_OP_PANTALLA_RECLAMAR` (`0x09`) en una
//! llamada a `bmo_valor`/`bmo_codigo`, y el DIRECTOR la lee ANTES de lanzar el
//! `.bex`: si esta, se aparta --suelta pantalla y entrada-- y espera a que el
//! programa la reclame o se muera.
//!
//! # Lo que se aprendio el 2026-09-11
//!
//! Un programa con LOS DOS caminos --`bmo_superficie_crear_con_buzon` si hay
//! quien componga, `bmo_pantalla_abrir` si no-- llevaba la bandera, porque
//! contiene el `0x09`. Lanzado desde el escritorio: el DIRECTOR se apartaba, el
//! programa pedia la VENTANA (nunca la pantalla), y el DIRECTOR se quedaba
//! **treinta segundos** esperando una reclamacion que no iba a llegar. Pantalla
//! negra, y luego la ventana. `ray.bex` y `doom.bex` estaban en ese caso.
//!
//! La bandera dice lo que el programa HACE. Y lo que hace un programa con los
//! dos caminos es preferir la ventana: **saber componerse** --preguntar por su
//! padre, `BMO_OP_MI_PADRE` (`0x26`), que es lo que hace `superficie/roja.h`
//! para ofrecer el bloque-- anula la bandera.
//!
//! Hasta hoy esta bandera no tenia NI UNA fila en el banco.

use super::*;

/// `bmo_valor` y `bmo_codigo` con la firma de `<bmo/bmo.h>`, sin incluirlo:
/// aqui se prueba la DEDUCCION del codegen, no la puerta.
const PUERTAS: &str = "unsigned long long bmo_valor(unsigned long long h, unsigned long long op, \
     unsigned long long a, unsigned long long b, unsigned long long c) { return h + op + a + b + c; } \
     long bmo_codigo(unsigned long long h, unsigned long long op, \
     unsigned long long a, unsigned long long b, unsigned long long c) { return (long)(h + op); } ";

/// Los `flags` de la cabecera BEF: offset 8, cuatro bytes little-endian.
fn flags_de(bef: &[u8]) -> u32 {
    u32::from_le_bytes([bef[8], bef[9], bef[10], bef[11]])
}

const WANTS_SCREEN: u32 = 1 << 10;

fn quiere_pantalla_el_programa(src: &str) -> bool {
    let bef = compile_source_to_bef(src).expect("tiene que compilar");
    flags_de(&bef) & WANTS_SCREEN != 0
}

fn quiere_pantalla(cuerpo: &str) -> bool {
    quiere_pantalla_el_programa(&format!("{PUERTAS} int main(void) {{ {cuerpo} return 0; }}"))
}

/// Solo la pantalla exclusiva: la bandera SE PONE. Es el DOOM de antes.
#[test]
fn solo_reclamar_la_pantalla_pone_la_bandera() {
    assert!(quiere_pantalla("bmo_valor(0, 0x09, 0, 0, 0);"));
    assert!(quiere_pantalla("bmo_codigo(0, 0x09, 0, 0, 0);"));
}

/// *** LA FILA QUE FALTABA: los dos caminos NO ponen la bandera. Es el orden
/// de `raycaster_C.c` y del DOOM de hoy: primero el padre, luego la pantalla.
#[test]
fn con_los_dos_caminos_no_se_pone() {
    assert!(!quiere_pantalla(
        "unsigned long long padre; padre = bmo_valor(0, 0x26, 0, 0, 0); \
         if (padre == 0) { bmo_valor(0, 0x09, 0, 0, 0); }"
    ));
    // Y el orden en el fuente no importa: es lo que el programa CONTIENE.
    assert!(!quiere_pantalla("bmo_valor(0, 0x09, 0, 0, 0); bmo_valor(0, 0x26, 0, 0, 0);"));
}

/// Ni pantalla ni padre: nada. Y solo el padre --una app que solo se compone--
/// tampoco: no hay pantalla que apartar.
#[test]
fn sin_reclamar_no_hay_bandera() {
    assert!(!quiere_pantalla("bmo_valor(0, 0x01, 0, 0, 0);"));
    assert!(!quiere_pantalla("bmo_valor(0, 0x26, 0, 0, 0);"));
}

/// El `0x09` en OTRA funcion no significa nada: solo cuentan las dos puertas.
#[test]
fn el_numero_en_otra_funcion_no_cuenta() {
    assert!(!quiere_pantalla_el_programa(
        "int f(int a, int b) { return a + b; } int main(void) { f(0, 0x09); return 0; }"
    ));
}
