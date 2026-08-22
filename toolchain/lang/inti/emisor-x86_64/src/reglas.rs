//! `reglas` -- LO QUE CUESTA NO TENER COMPORTAMIENTO INDEFINIDO, en bytes.
//!
//! ## Por que soy un fichero y no un trozo del de al lado (L6b)
//!
//! Porque contesto *"como se comprueba que esto no se salio"*, y esa pregunta
//! tiene dueno: las doce reglas de `REGLAS.md`. Un fichero aparte deja que se
//! lean juntas y que se cuente lo que ocupan.
//!
//! ** De las cuatro comprobaciones de la IR, tres llegan a bytes y solo UNA
//! necesita mas de cuatro instrucciones -- la conversion, que vive aqui entera.
//! Las otras dos caben al lado de la operacion que las pide y se quedan en
//! `emitir_funcion`, que es donde se ve que van pegadas a ella.
//!
//! OJO: la Regla 2 no esta, y no es un olvido de este fichero. Un `bufer` no
//! lleva su longitud, asi que no hay contra que comprobar; nace con `lista de
//! T`. Cuando llegue, su sitio es aqui.

use bmo_inti_front::ir::Valor;
use bmo_lower::x86;

use crate::marco::Marco;
use crate::{carga, DER, IZQ};

/// LA REGLA 12 EN BYTES: cabe este numero de coma flotante en tantos bytes?
///
/// ## ** Por que hacen falta DOS preguntas y no una
///
/// La instruccion que trunca **no avisa cuando el numero no cabe**: devuelve el
/// entero mas negativo como centinela y sigue. Y ese centinela es ambiguo,
/// porque tambien es el resultado legitimo de convertir exactamente `-2^63`.
///
/// ```text
///    1. es el centinela?   -> puede ser desborde... o el numero de verdad
///       si lo es, se compara el ORIGINAL con -2^63 exacto:
///          iguales      -> era legitimo, sigue
///          distintos    -> desbordo, o era NaN. Atrapa
///    2. cabe en n bytes?   -> extender los bajos con signo y comparar
/// ```
///
/// ## ** Y el NaN, que es el caso que se cuela sin la segunda bandera
///
/// Un NaN truncado da tambien el centinela. Al compararlo con `-2^63` sale
/// "no comparable", que enciende la bandera de igualdad **a la vez** que la de
/// paridad. Sin mirar la segunda, `entero64(0.0 / 0.0)` pasaria por legitimo.
///
/// ## Lo que cuesta en el camino que NO atrapa
///
/// Dos comparaciones y dos saltos que no saltan. El bloque del centinela se
/// esquiva entero con el primer salto, asi que un programa normal paga tres
/// instrucciones -- y un procesador fuera de orden las predice todas, porque
/// nunca saltan.
pub(crate) fn regla_doce(
    out: &mut Vec<u8>,
    sobre: &Valor,
    bytes: u32,
    marco: &Marco,
    huecos: &mut Vec<(usize, u64)>,
    codigo: u64,
) {
    // El original, en el registro de coma flotante, y el truncado en el de
    // trabajo. La instruccion no toca el original: hace falta entero mas abajo.
    carga(out, IZQ, sobre, marco);
    x86::movq_xmm_de_r64(out, 0, IZQ);
    x86::cvttsd2si_r64(out, IZQ);

    // -- 1. es el centinela?
    x86::mov_r64_imm64(out, DER, 0x8000_0000_0000_0000);
    x86::cmp_r64_r64(out, IZQ, DER);
    // Si NO lo es, la conversion de 64 bits fue limpia: al ancho directamente.
    let al_ancho = x86::salto_corto(out, 0x75); // jne

    // Lo es. El unico original que puede darlo legitimamente es `-2^63`, cuyo
    // patron de bits se escribe entero para que se pueda comparar con el manual.
    x86::mov_r64_imm64(out, DER, 0xC3E0_0000_0000_0000);
    x86::movq_xmm_de_r64(out, 1, DER);
    x86::comisd(out);
    // No comparable (NaN) -> fuera.
    out.extend_from_slice(&[0x0F, 0x8A]); // jp
    huecos.push((out.len(), codigo));
    out.extend_from_slice(&[0, 0, 0, 0]);
    // Comparable y distinto de -2^63 -> desbordo.
    out.extend_from_slice(&[0x0F, 0x85]); // jne
    huecos.push((out.len(), codigo));
    out.extend_from_slice(&[0, 0, 0, 0]);

    x86::cierra_salto_corto(out, al_ancho);

    // -- 2. cabe en `bytes`?
    //
    // ** En ocho no hay nada que preguntar: lo que cupo en el truncado cabe en
    // el destino, y el unico caso raro --el centinela-- ya se resolvio arriba.
    if bytes >= 8 {
        return;
    }
    match bytes {
        4 => x86::movsxd_r64_r32(out, DER, IZQ),
        2 => x86::movsx_r64_r16(out, DER, IZQ),
        _ => x86::movsx_r64_r8(out, DER, IZQ),
    }
    x86::cmp_r64_r64(out, IZQ, DER);
    // Si extender los bajos con signo no devuelve el mismo numero, es que los
    // altos llevaban algo -- o sea, no cabia.
    out.extend_from_slice(&[0x0F, 0x85]); // jne
    huecos.push((out.len(), codigo));
    out.extend_from_slice(&[0, 0, 0, 0]);
}
