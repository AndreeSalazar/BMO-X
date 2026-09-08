//! **EL PULSO DEL ESCRITORIO: cuantas vueltas da por segundo, siempre a la vista.**
//!
//! # *** POR QUE EXISTE, y es una leccion sobre diagnosticar
//!
//! El 2026-09-08 el dueno trajo un sintoma exacto:
//!
//! > *"los FPS dependen de un teclado que no tiene sentido... tengo que pulsar
//! > el bloq numerico SOLO para ver 1 frame que cambia"*
//!
//! Y se persiguieron **SEIS hipotesis leyendo codigo**, una detras de otra:
//!
//! ```text
//!    el framebuffer sin write-combining    NO -- `map_page_wc` existe y se usa
//!    el hilo del bus muerto de hambre      NO -- prioridad 2 contra 0
//!    `yield_screen()` bloqueando           NO -- es un yield, deja LISTA
//!    el `sfence` que falta                 NO -- esta al final de compose
//!    el planificador por prioridad         NO -- el bus gana siempre
//!    `gather` esperando una tecla          NO -- sondea, no bloquea
//! ```
//!
//! Seis descartes correctos y **cero avance**, porque todos contestaban a la
//! misma pregunta y ninguno la contestaba de verdad:
//!
//! > **El bucle del escritorio, gira o no gira?**
//!
//! *** Y esa pregunta tenia respuesta desde hacia meses --`Tick::loops_per_second`
//! se calcula con el TSC en cada vuelta-- pero solo se pintaba **dentro de las
//! ventanas de CPU y MEMORIA**, que hay que ABRIR CON UNA TECLA.
//!
//! > El unico numero que dice si el escritorio esta vivo estaba detras de la
//! > cosa cuya muerte habia que diagnosticar.
//!
//! Es la misma leccion que ya obligo a poner la ficha de CABINA siempre en la
//! barra, escrita en `paint.rs`: *"un panel de diagnostico al que solo se llega
//! con el aparato que puede estar roto no es un panel de diagnostico"*. Se
//! aprendio para CABINA y no se aplico al pulso.
//!
//! # Como se lee, y parte el problema EN DOS
//!
//! ```text
//!    miles          el bucle GIRA. El fallo esta en quien marca sucio
//!    decenas o 0    el bucle NO gira: se bloquea, y el fallo esta ANTES
//! ```
//!
//! ** Ese es todo su trabajo. No dice cual es el fallo: dice **en cual de las
//! dos mitades buscarlo**, y eso es lo que faltaba tras seis descartes.
//!
//! # Lo que NO hace
//!
//! ```text
//!    [ ] no mide FPS: mide VUELTAS. Una vuelta sin nada sucio no pinta
//!    [ ] no arregla nada
//!    [ ] y no se puede cerrar, igual que la ficha de CABINA
//! ```

use bmo_userland as bmo;

use super::{chip_box, INK, INK_DIM, TASKBAR};
use crate::text::decimal;

/// La ranura siguiente al testigo del USB. El testigo mide 168 px desde la
/// suya, asi que esto empieza pasado ese ancho.
const TRAS_TESTIGO: u32 = 168 + 8;
/// Lo que ocupa: `pulso 12345/s` mas margen.
const ANCHO: u32 = 132;

/// Lo ultimo que se pinto. Repintar el mismo numero cuatro veces por segundo
/// seria ensuciar una caja para que se vea igual -- y ensuciar es lo que
/// cuesta, porque `volcar` copia cajas sucias.
static mut ULTIMO: u32 = u32::MAX;

/// **Olvida lo pintado.** Lo llama quien repinta la barra entera: si no, la
/// caja queda tapada y este modulo cree que sigue en pantalla.
pub(crate) fn olvidar() {
    unsafe { ULTIMO = u32::MAX };
}

/// **Pinta el pulso si cambio.** Se llama en las vueltas del cuarto de segundo.
pub(crate) fn refrescar(p: &bmo::Pantalla, vueltas: u32) {
    unsafe {
        if ULTIMO == vueltas {
            return;
        }
        ULTIMO = vueltas;
    }
    let (x0, y, _, h) = chip_box(super::testigo::RANURA);
    let x = x0 + TRAS_TESTIGO;
    // En una pantalla estrecha no cabe, y se prefiere no pintarlo a pintarlo
    // encima de otra cosa. Misma regla que el testigo.
    if x + ANCHO >= p.ancho {
        return;
    }
    p.rect(x, y, ANCHO, h, TASKBAR);
    let ty = y + (h.saturating_sub(bmo::GLIFO_ALTO)) / 2;
    let tx = p.texto(x + 4, ty, "pulso ", INK_DIM);
    let mut buf = [0u8; 10];
    let n = decimal(vueltas as u64, &mut buf);
    // ** EN BLANCO SI ES BAJO. Un pulso de dos digitos no es un detalle de
    // rendimiento: es el bucle bloqueado, y tiene que llamar la atencion sin
    // que nadie sepa que numero esperar.
    let tinta = if vueltas < 100 { INK } else { INK_DIM };
    let tx = p.texto_bytes(tx, ty, &buf[..n], tinta);
    p.texto(tx, ty, "/s", INK_DIM);
}
