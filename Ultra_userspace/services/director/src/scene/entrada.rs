//! **DONDE EMPIEZA LA LATENCIA**: el ritmo del bus de entrada, a la vista.
//!
//! [consumo] NADA      no corre en reposo por su cuenta: pinta cuando el
//!                     compositor se lo pide, y el compositor solo pinta si
//!                     algo cambio (L6h)
//!
//! # Por que existe, y es la SEPTIMA vez que se escribe esta frase
//!
//! El camino de la mano al pixel empieza en el hilo del bus USB, y ese hilo late
//! cada `BUS_PERIOD_MS = 4`. O sea que **antes de que BMO-X sepa siquiera que el
//! raton se movio ya pueden haber pasado 4 ms**, y eso no lo arregla ningun
//! compositor por rapido que sea.
//!
//! *** Los dos numeros que dicen eso existen desde hace semanas --`ritmo()` y
//! `peor_trabajo()` en `dev/usb/bus.rs`-- y hasta el 09-09 **los leia un solo
//! sitio: `cabina/cockpit.rs`, que es una pantalla de RING 0**. Y de Ring 0 no
//! se vuelve: el dueno vive en el escritorio.
//!
//! ```text
//!    CABINA            se veia solo con una tecla
//!    el testigo USB    igual
//!    el pulso          solo dentro de la ventana de CPU
//!    el volcado        solo con la orden `mem` del shell
//!    el modo del lienzo  solo por la consola del arranque, que se tapa
//!    `cuerpo`          medido y sin leer
//!    -> y este, el septimo
//! ```
//!
//! > Un instrumento al que hay que ir no se mira. El que esta delante, si.
//!
//! # Que se lee aqui, y es un juicio de AFORO
//!
//! ```text
//!    entrada 4ms 120us/s purga
//!            |     |        |
//!            |     |        +-- cual de los cinco trabajos de la vuelta
//!            |     +-- lo peor que tardo uno de ellos EN EL ULTIMO SEGUNDO
//!            +-- cada cuanto late el bus: EL SUELO de la latencia
//!
//! *** `/s` y no a secas, porque el primer arranque enseno por que hace falta:
//! salio `7666us` --el 192% del periodo-- y era el ARRANQUE enumerando el USB.
//! Un maximo desde el arranque contesta *"paso alguna vez"* a una pregunta que
//! es *"esta pasando"*. Ahora la ventana es de un segundo. Ver `dev/usb/bus.rs`.
//! ```
//!
//! ** `peor` se enciende en blanco cuando pasa del **80% del periodo**, y eso no
//! es estetica: si un trabajo de la vuelta se acerca a lo que dura la vuelta
//! entera, el hilo **no puede mantener su ritmo**. Es `C/T` acercandose a 1, la
//! misma cuenta que `PLAN_EL_COMPAS` le hace a las tareas -- aplicada al hilo
//! que hoy decide la latencia de todo el sistema.
//!
//! [!] Y `purga` va a salir alta casi siempre: cede el CPU hasta ocho veces
//! esperando a `reap`. No es un fallo suyo, es lo que hace. Se ensena igual --
//! tapar un numero porque se sabe explicar es como se pierden los datos.
//!
//! # Lo que NO dice, y hace falta saberlo
//!
//! ```text
//!    [ ] no dice el `bInterval` que pidio el raton. El descriptor lo trae y
//!        `uhid/enumera.rs` LO LEE --se lo pasa al Endpoint Context y lo
//!        escribe en el log-- pero no sube a Ring 3. Hasta que suba, no se
//!        puede saber si esos 4 ms sobran o son justos
//!    [ ] no mide la latencia de punta a punta: mide su PRIMER SUMANDO. Los
//!        otros son el despertar del compositor (<=1 ms) y el escaner de video
//!        (<=16,7 ms, y sin V-Sync). Ver `docs/plan/PLAN_EL_PIXEL.md`
//! ```

use bmo_userland as bmo;

use super::huella::{cambio, Huella};
use super::{chip_box, INK, INK_DIM, TASKBAR};
use crate::text::decimal;

/// Detras del volcado, que ocupa 300 px desde `TRAS_PULSO`.
const TRAS_VOLCADO: u32 = 176 + 400 + 8 + 300 + 8;
/// Lo que ocupa: `entrada 4ms peor 120us purga` mas margen.
const ANCHO: u32 = 250;

/// Los cinco trabajos de una vuelta del bus, en el orden de `dev/usb/bus.rs`.
///
/// ** Es una SEGUNDA COPIA de `NOMBRES`, y hay que decirlo: el kernel manda el
/// indice, no el nombre, porque por la puerta caben numeros y no cadenas. Si
/// alguien anade un sexto trabajo alli y no aqui, esta tabla dira el nombre
/// equivocado -- por eso hay un `?` para el indice que no conoce, en vez de
/// recortar el indice y ensenar siempre el ultimo.
const TRABAJOS: [&str; 8] = [
    "bombeo", "rescate", "emerg", "purga", "radar",
    // ** Los tres de DENTRO de `bombeo`, desde el 09-09. El metal dijo
    // `7781us/s bombeo` y `bombeo` son cuatro trabajos en uno: preguntarle al
    // total cual tarda es preguntarle al total. Ver `dev/usb/bus.rs`.
    "anillo", "audio", "salud",
];

/// Lo ultimo que se pinto. Ver [`super::huella`].
static mut HUELLA: Huella = Huella::nueva();

/// **Olvida lo pintado.** Lo llama [`super::olvidar_la_barra`].
pub(crate) fn olvidar() {
    super::huella::olvidar(unsafe { &mut *core::ptr::addr_of_mut!(HUELLA) });
}

/// **Pinta el suelo de la latencia.** Se llama en las vueltas que pintan.
pub(crate) fn refrescar(p: &bmo::Pantalla) {
    let ritmo = bmo::info(bmo::INFO_USB_RITMO);
    // La firma ES el dato entero: no hay nada que se pinte y no venga de aqui.
    // Cuando una firma se calcula a partir de MENOS de lo que se pinta, el chip
    // se congela sin decirlo -- el `[riesgo] SILENCIO` de la casa.
    if !cambio(unsafe { &mut *core::ptr::addr_of_mut!(HUELLA) }, ritmo) {
        return;
    }

    let (x0, y, _, h) = chip_box(super::testigo::ranura());
    let x = x0 + TRAS_VOLCADO;
    // Misma regla que sus tres vecinos: si no cabe, no se pinta. Pintar encima
    // de otra cosa es peor que no pintar.
    if x + ANCHO >= p.ancho {
        return;
    }
    // ** La raya que lo separa del instrumento de la izquierda. Sin ella los
    // tres se leen como un solo parrafo de numeros. Ver `scene::SEPARADOR`.
    p.rect(x - 5, y + 4, 1, h.saturating_sub(8), crate::scene::SEPARADOR);
    p.rect(x, y, ANCHO, h, TASKBAR);
    let ty = y + (h.saturating_sub(bmo::GLIFO_ALTO)) / 2;
    let tx = p.texto(x + 4, ty, "entrada ", INK_DIM);

    let periodo_ms = ritmo & 0xFFFF;
    let peor_us = (ritmo >> 16) & 0xFFFF_FFFF;
    let cual = ((ritmo >> 48) & 0xFF) as usize;

    // Sin periodo no hay hilo de bus, y entonces no hay nada mas que decir: los
    // otros dos campos serian ceros con cara de dato.
    if periodo_ms == 0 {
        p.texto(tx, ty, "SIN BUS", INK);
        return;
    }

    // Diez EXACTOS: es lo que pide `decimal`, y `peor_us` es de 32 bits,
    // o sea 10 digitos como mucho. Justo, y por eso queda dicho.
    let mut buf = [0u8; 10];
    let n = decimal(periodo_ms, &mut buf);
    let tx = p.texto_bytes(tx, ty, &buf[..n], INK_DIM);
    let tx = p.texto(tx, ty, "ms ", INK_DIM);

    let n = decimal(peor_us, &mut buf);
    // ** EL AFORO DEL HILO DEL BUS: si un solo trabajo se come el 80% de la
    // vuelta, el ritmo no se puede sostener. `C/T` acercandose a 1.
    let tinta = if peor_us * 10 >= periodo_ms * 1000 * 8 { INK } else { INK_DIM };
    let tx = p.texto_bytes(tx, ty, &buf[..n], tinta);
    let tx = p.texto(tx, ty, "us/s ", INK_DIM);

    // El indice que esta tabla no conoce se dice, no se recorta: un `?` manda a
    // mirar `dev/usb/bus.rs`; recortarlo ensenaria `radar` para siempre.
    let nombre = if cual < TRABAJOS.len() { TRABAJOS[cual] } else { "?" };
    p.texto(tx, ty, nombre, INK_DIM);
}
