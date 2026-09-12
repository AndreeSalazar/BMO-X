//! **CARRIL VERDE** -- donde caen los pixeles. Se puede tocar sin miedo.
//!
//! [consumo] NADA      no corre en reposo por su cuenta: pinta cuando el
//!                     compositor se lo pide, y el compositor solo pinta si
//!                     algo cambio (L6h)
//!
//! [carril]  VERDE     no decide nada: recibe el `Dictamen` ya resuelto y lo
//!           pone en pantalla. Equivocarse aqui se VE
//!
//! [cuesta]  NADA -- son coordenadas, anchos y tintas de una caja de la barra
//!           de tareas. Un fallo aqui deja la caja torcida, corrida o sin
//!           pintar, y se nota en el acto porque **la esta mirando el dueno**.
//!           No puede mentir: para mentir hay que afirmar algo, y aqui no se
//!           afirma nada.
//!
//! [riesgo]  -- ninguno declarado. Es el carril que no tiene letrero, y por eso
//!           existe el fichero: **saber que algo es verde tambien es saber**.
//!
//! # Por que esto NO esta con lo otro
//!
//! Porque las dos averias del pulso fueron las dos de significado, y ninguna de
//! sitio. Mientras las dos cosas vivian en la misma funcion, un cambio de donde
//! cae un numero y un cambio de que dice ese numero se leian igual en el diff.
//!
//! ```text
//!    amarilla.rs   que es verdad     se equivoca CALLANDO   -> con dos manos
//!    verde.rs      donde va          se equivoca A LA VISTA -> se puede jugar
//! ```
//!
//! # El unico comportamiento suyo que hay que explicar
//!
//! **Se encoge antes de callarse.** La regla vieja era *"si no cabe, no se
//! pinta"*, copiada del testigo. Con la caja larga eso apagaba el pulso ENTERO
//! en una pantalla estrecha: se perderia **lo que dice si el escritorio esta
//! vivo** por no caber **lo que dice en que se le va el tiempo**.
//!
//! ```text
//!    cabe entero   latido 1000/s  pinta 4  cuerpo 40  puerta 950  |
//!    cabe corto    latido 1000/s  |
//!    no cabe       nada -- pintar encima de otra cosa es peor que no pintar
//! ```
//!
//! Primero lo que no se puede perder. El reparto es lo que sobra si falta sitio.

use bmo_userland as bmo;

use super::amarilla::Dictamen;
use crate::scene::{chip_box, INK, INK_DIM, TASKBAR};
use crate::text::decimal;

/// La ranura siguiente al testigo del USB. El testigo mide 168 px desde la
/// suya, asi que esto empieza pasado ese ancho.
const TRAS_TESTIGO: u32 = 168 + 8;
/// Lo que ocupa la forma LARGA:
/// `latido 12345/s  pinta 4  cuerpo 900  puerta 40  |`.
const ANCHO: u32 = 400;
/// Lo que ocupa la forma CORTA: `pulso 12345/s |`, sin el reparto.
const ANCHO_CORTO: u32 = 150;

/// **Pintar lo ya decidido.** Aqui no se pregunta nada: llega resuelto.
pub(crate) fn pintar(p: &bmo::Pantalla, d: &Dictamen) {
    let (x0, y, _, h) = chip_box(super::super::testigo::RANURA);
    let x = x0 + TRAS_TESTIGO;
    // ** SE ENCOGE ANTES DE CALLARSE. Ver la cabecera: el orden de lo que se
    // sacrifica no es de gusto, es de para que existe la caja.
    let ancho = if x + ANCHO < p.ancho {
        ANCHO
    } else if x + ANCHO_CORTO < p.ancho {
        ANCHO_CORTO
    } else {
        return;
    };
    // ** La raya que lo separa del testigo. Sin ella los instrumentos se leen
    // como un solo parrafo de numeros. Ver `scene::SEPARADOR`.
    p.rect(x - 5, y + 4, 1, h.saturating_sub(8), crate::scene::SEPARADOR);
    p.rect(x, y, ancho, h, TASKBAR);
    let ty = y + (h.saturating_sub(bmo::GLIFO_ALTO)) / 2;
    // ** EL NOMBRE DICE EL MODO. No es adorno: si el kernel no dio el latido,
    // el bucle gira igual de bien y las dos formas se verian identicas. Ver la
    // decision 6 de `amarilla.rs`.
    let modo = if d.en_reposo { "reposo " } else if d.en_latido { "latido " } else { "pulso " };
    let tx = p.texto(x + 4, ty, modo, INK_DIM);
    let mut tx = match d.ritmo {
        None => p.texto(tx, ty, "SIN RELOJ ", INK),
        Some(v) => {
            let mut buf = [0u8; 10];
            let n = decimal(v as u64, &mut buf);
            let tinta = if d.alarma { INK } else { INK_DIM };
            let tx = p.texto_bytes(tx, ty, &buf[..n], tinta);
            p.texto(tx, ty, "/s ", INK_DIM)
        }
    };
    // El reparto solo con reloj --son ms, y sin reloj no hay ms-- y solo si
    // cabe entero. La mitad que manda va en blanco: es la respuesta, y tiene
    // que verse sin leer los numeros.
    if d.ritmo.is_some() && ancho == ANCHO {
        // `pinta` NUNCA en blanco: no es una alarma, es la escala con la que se
        // lee el ritmo de al lado. Ponerla a competir con la mitad que manda
        // seria gastar el unico color fuerte de la caja en dos cosas.
        tx = renglon(p, tx, ty, " pinta ", d.pinta, false);
        tx = renglon(p, tx, ty, " cuerpo ", d.cuerpo_ms, d.manda_cuerpo);
        tx = renglon(p, tx, ty, " puerta ", d.puerta_ms, !d.manda_cuerpo);
    }
    // La aguja al final, en blanco: es lo unico de esta caja que tiene que
    // verse desde lejos sin leer.
    p.texto_bytes(tx + 4, ty, &[d.aguja], INK);
}

/// Una mitad del reparto: su nombre en gris y su numero en blanco si es la que
/// se queda el segundo.
fn renglon(p: &bmo::Pantalla, x: u32, y: u32, nombre: &str, ms: u32, manda: bool) -> u32 {
    let tx = p.texto(x, y, nombre, INK_DIM);
    let mut buf = [0u8; 10];
    let n = decimal(ms as u64, &mut buf);
    p.texto_bytes(tx, y, &buf[..n], if manda { INK } else { INK_DIM })
}
