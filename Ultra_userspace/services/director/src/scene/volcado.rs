//! **LO QUE CUESTA EMPUJAR UN FOTOGRAMA A LA PANTALLA**, siempre a la vista.
//!
//! # *** POR QUE EXISTE, y es la MISMA leccion por cuarta vez (2026-09-09)
//!
//! El 08-09, con el reloj de CPU por fin midiendo trabajo y no reloj de pared,
//! salio el numero que la sesion entera venia persiguiendo:
//!
//! ```text
//!    moviendo el raton   latido 20/s   pinta 20   cuerpo 795
//!    -> 795 ms de CPU entre 20 pintados = 39,7 ms POR FOTOGRAMA
//!    -> y el presupuesto a 60 Hz son 16,7. Un pintado cuesta 2,4 fotogramas
//! ```
//!
//! ** Pero ese numero no dice si son 40 KB movidos muy despacio o 8 MB movidos
//! a la velocidad normal, **y el arreglo es completamente distinto**: lo primero
//! es el coste por pixel, lo segundo es que el troceado por cajas degenero.
//!
//! *** Y la respuesta llevaba meses medida y sin leer. `Pantalla::volcado()`
//! existe desde el 12-08 y hasta hoy solo la miraba la orden `mem` del shell --
//! o sea que **para verla habia que abrir una ventana con una tecla**, que es
//! exactamente el fallo que ya obligo a poner CABINA, el testigo del USB y el
//! pulso en la barra. Van cuatro.
//!
//! > Un instrumento al que hay que ir no se mira. El que esta delante, si.
//!
//! # Como se lee, y lo dice su propio autor
//!
//! La cabecera de `Volcado::cajas` ya dejo escrito el diagnostico entero:
//!
//! ```text
//!    peor pequeno  + cajas 2 o 3   el troceado TRABAJA
//!    peor de 8 MB  + cajas 1       degenero: se vuelca la pantalla entera,
//!                                  y el sospechoso es `COSTE_DE_UNA_CAJA`
//! ```
//!
//! Por eso se pintan **esos dos y no otros**: son el par que decide, y cualquier
//! tercero solo competiria por el sitio.
//!
//! # Lo que NO hace
//!
//! ```text
//!    [ ] no mide tiempo: mide BYTES. El tiempo lo dice `cuerpo` del pulso,
//!        y hacen falta los dos -- uno sin el otro no distingue "mucho" de
//!        "lento"
//!    [ ] `peor` no baja nunca: es el peor caso desde el arranque, y esta
//!        bien que sea asi. Un maximo que se olvida no es un maximo
//! ```

use bmo_userland as bmo;

use super::{chip_box, INK, INK_DIM, TASKBAR};
use crate::text::decimal;

/// Va detras del pulso, que ocupa 400 px desde `TRAS_TESTIGO`.
const TRAS_PULSO: u32 = 176 + 400 + 8;
/// Lo que ocupa: `volcado 8192K cajas 1` mas margen.
const ANCHO: u32 = 250;

/// Por encima de esto, el volcado dejo de ser troceado y es la pantalla entera.
///
/// ** No es un umbral de gusto: 1 MiB por fotograma a 60 Hz son 60 MB/s, y el
/// blit medido va a ~300 MB/s. O sea que a partir de aqui el volcado **solo**
/// ya se come un quinto del presupuesto. Ver `bmo-compositor-escaner`.
const PEOR_QUE_GRITA_KIB: u64 = 1024;

/// **Pinta lo que cuesta el peor fotograma.** Se llama en las vueltas que pintan.
pub(crate) fn refrescar(p: &bmo::Pantalla, v: &bmo::Volcado) {
    let (x0, y, _, h) = chip_box(super::testigo::RANURA);
    let x = x0 + TRAS_PULSO;
    // Misma regla que el pulso y el testigo: si no cabe, no se pinta. Pintar
    // encima de otra cosa es peor que no pintar.
    if x + ANCHO >= p.ancho {
        return;
    }
    p.rect(x, y, ANCHO, h, TASKBAR);
    let ty = y + (h.saturating_sub(bmo::GLIFO_ALTO)) / 2;
    let tx = p.texto(x + 4, ty, "volcado ", INK_DIM);

    // ** SIN UN SOLO FOTOGRAMA NO SE PINTA UN CERO. Un cero aqui se leeria
    // como "no cuesta nada", y lo que pasa es que todavia no ha volcado nadie.
    // Es la leccion del `pulso 0/s` de ayer, un modulo mas alla.
    if v.fotogramas == 0 {
        p.texto(tx, ty, "sin volcar", INK_DIM);
        return;
    }

    let peor_kib = v.peor / 1024;
    let mut buf = [0u8; 10];
    let n = decimal(peor_kib, &mut buf);
    // En blanco cuando el peor fotograma ya no es un troceado sino la pantalla.
    let tinta = if peor_kib >= PEOR_QUE_GRITA_KIB { INK } else { INK_DIM };
    let tx = p.texto_bytes(tx, ty, &buf[..n], tinta);
    let tx = p.texto(tx, ty, "K cajas ", INK_DIM);

    let n = decimal(v.cajas as u64, &mut buf);
    // ** Y `cajas 1` con un peor grande es el diagnostico completo: el troceado
    // degenero. Por eso las dos van juntas y ninguna sola sirve.
    let tinta = if v.cajas <= 1 && peor_kib >= PEOR_QUE_GRITA_KIB { INK } else { INK_DIM };
    p.texto_bytes(tx, ty, &buf[..n], tinta);
}
