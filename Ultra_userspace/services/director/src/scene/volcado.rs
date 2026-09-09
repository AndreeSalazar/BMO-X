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

use super::huella::{cambio, Huella};
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

/// Lo ultimo que se pinto. Ver [`super::huella`].
static mut HUELLA: Huella = Huella::nueva();

/// **Olvida lo pintado.** Lo llama [`super::olvidar_la_barra`].
pub(crate) fn olvidar() {
    super::huella::olvidar(unsafe { &mut *core::ptr::addr_of_mut!(HUELLA) });
}

/// Todo lo que esta caja ensena, en un numero.
///
/// ** Los cuatro campos, y ni uno menos: una firma que se deja fuera algo que SI
/// se pinta congela el chip sin decirlo. `peor` va en KiB porque es lo que se
/// ensena --los bytes de mas no cambian el dibujo-- y `fotogramas` entra solo
/// como *"hubo alguno"*, que es la unica pregunta que este chip le hace.
fn firma(v: &bmo::Volcado) -> u64 {
    let ninguno = matches!(v.modo, bmo::Volcador::Ninguno) as u64;
    ((v.peor / 1024) << 8) | ((v.cajas as u64 & 0x1F) << 3) | (ninguno << 1)
        | (v.fotogramas != 0) as u64
}

/// **Pinta lo que cuesta el peor fotograma.** Se llama en las vueltas que pintan.
pub(crate) fn refrescar(p: &bmo::Pantalla, v: &bmo::Volcado) {
    // ** NO SE REPINTA LO QUE YA ESTA. `peor` es un maximo --sube y se queda-- y
    // `cajas` cambia con el. O sea que este chip cambia unas pocas veces en toda
    // una sesion y se estaba redibujando en cada fotograma que pinta: 6.000
    // pixeles por vuelta para ensenar el mismo numero. Ver `super::huella`.
    if !cambio(unsafe { &mut *core::ptr::addr_of_mut!(HUELLA) }, firma(v)) {
        return;
    }
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

    // == *** EL MODO VA PRIMERO, Y ES EL HECHO MAS DECISIVO (2026-09-09) ===
    //
    // La primera version de este fichero pintaba `peor` y `cajas` y **no el
    // modo**, y con eso escondia justo lo que mas importa saber:
    //
    // ```text
    //    Directo   hay lienzo -> se dibuja en RAM y se vuelca la caja sucia
    //    NINGUNO   no hay lienzo -> cada rect y cada glifo va DIRECTO a la
    //              VRAM por PCIe, y `read()` LEE de la VRAM. Cien veces mas
    //              caro, y el escritorio funciona igual de bien
    // ```
    //
    // ** Y en modo `Ninguno` la trampa era doble: `volcar` sale por su `return`
    // ANTES de contar, asi que `fotogramas` se queda en 0 para siempre y esta
    // caja decia `sin volcar` -- que se lee como *"todavia nada"* y no como
    // *"no hay lienzo"*. Dos estados muy distintos con el mismo texto: la
    // misma clase de fallo que el `pulso 0/s`, y en el fichero de al lado.
    //
    // *** El arranque SI lo dice --`SIN doble bufer: no hubo bloque, pinto
    // directo al panel`-- por la CONSOLA, que se va y la tapa el escritorio.
    // Van SEIS instrumentos que decian la verdad donde nadie mira.
    if matches!(v.modo, bmo::Volcador::Ninguno) {
        // En blanco: no es un detalle de rendimiento, es cien veces el coste.
        p.texto(tx, ty, "SIN LIENZO", INK);
        return;
    }

    // Sin un solo fotograma no se pinta un cero. Aqui ya se sabe que hay
    // lienzo, asi que esto significa lo que parece: no ha volcado nadie aun.
    if v.fotogramas == 0 {
        p.texto(tx, ty, "aun nada", INK_DIM);
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
