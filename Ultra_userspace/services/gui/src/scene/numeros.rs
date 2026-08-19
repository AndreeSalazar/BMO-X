//! **LA PESTANA `numeros`**: como esta el almacen, de un vistazo.
//!
//! === Por que es un fichero y no un trozo de `data.rs` ===
//!
//! Por L6a: `data.rs` paso de las mil lineas y el censo dijo que no. Pero el
//! corte no se eligio por tamano -- se eligio porque **este trozo no comparte
//! NADA con el explorador**.
//!
//! ```text
//!   [numeros]      contesta "COMO ESTA el almacen"  -- generacion, sitio,
//!                  identidad, nivel. No mira ni un nodo.
//!   [explorador]   contesta "QUE HAY dentro"        -- arbol, rejilla, grafo.
//!                  No mira ni un numero del volumen.
//! ```
//!
//! Ni una funcion de aqui la llama el otro lado, y al reves tampoco. `level_text`
//! y `magnitude` solo se usaban aqui y se vienen con el: **el corte se elige por
//! nombres libres**, y estos dos lo estaban.
//!
//! === Lo que ensena, y por que en este orden ===
//!
//! Generacion primero, porque es lo que cambia al escribir y por tanto lo
//! primero que uno viene a mirar despues de sellar. Despues el sitio, la
//! identidad del disco y el nivel de ocupacion -- que es el que puede decir
//! SOLO LECTURA y por eso va con su color.

use bmo_userland as bmo;

use super::data::DataWindow;
use super::*;
use crate::text::decimal;

/// Los cuatro niveles de `bmo_estratos::espacio`, con su color.
///
/// El orden es el del ABI (`INFO_ES_NIVEL`), no uno inventado aqui: si
/// divergieran, el panel pintaria en verde un volumen en solo lectura.
fn level_text(n: u64) -> (&'static str, u32) {
    match n {
        0 => ("holgado", INK_OK),
        1 => ("AVISO: por encima del 70%", 0x00F0_D070),
        2 => ("FAULT: por encima del 85%", INK_BAD),
        _ => ("SOLO LECTURA: por encima del 95%", INK_BAD),
    }
}

/// Pinta la pestana de numeros dentro de la ventana.
///
/// `tx` es el margen izquierdo que ya calculo el marco: se recibe en vez de
/// recalcularlo para que las dos pestanas empiecen en la misma columna. Dos
/// margenes distintos en la misma ventana se ven como dos programas.
pub(crate) fn paint(p: &bmo::Pantalla, c: &DataWindow, tx: u32) {
    let mut ty = c.chrome.y + TITLE_H + 6;

    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        p.texto(tx, ty, "ningun volumen ESTRATOS montado.", INK_BAD);
        ty += bmo::GLIFO_ALTO + 4;
        p.texto(tx, ty, "se formatea desde el anfitrion con estratos-fmt.", INK_DIM);
        return;
    }

    let bloques = bmo::info(bmo::INFO_ES_BLOQUES);
    let used_count = bmo::info(bmo::INFO_ES_USADOS);
    let tam = bmo::info(bmo::INFO_ES_BLOQUE_TAM).max(1);
    let level = bmo::info(bmo::INFO_ES_NIVEL);

    let row = |label: &str, y: &mut u32, pinta: &dyn Fn(u32, u32)| {
        p.texto(tx, *y, label, INK_DIM);
        pinta(tx + 13 * bmo::GLIFO_ANCHO, *y);
        *y += bmo::GLIFO_ALTO + 3;
    };

    row("generacion", &mut ty, &|x, y| {
        let g = bmo::info(bmo::INFO_ES_GENERACION);
        let mut b = [0u8; 10];
        let n = decimal(g, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], INK);
        p.texto(x, y, "  transacciones desde el formateo", INK_DIM);
    });

    row("espacio", &mut ty, &|x, y| {
        let x = magnitude(p, x, y, used_count * tam, INK);
        let x = p.texto(x, y, " de ", INK_DIM);
        let x = magnitude(p, x, y, bloques * tam, INK);
        let pct = if bloques == 0 { 0 } else { used_count * 100 / bloques };
        let x = p.texto(x, y, "   ", INK_DIM);
        let mut b = [0u8; 10];
        let n = decimal(pct, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], INK);
        p.texto(x, y, "%", INK);
    });

    row("estado", &mut ty, &|x, y| {
        let (t, color) = level_text(level);
        p.texto(x, y, t, color);
    });

    row("identidad", &mut ty, &|x, y| {
        if bmo::info(bmo::INFO_ES_IDENTIDAD) != 0 {
            p.texto(x, y, "nacio en ESTE disco", INK_OK);
        } else {
            p.texto(x, y, "NO nacio aqui: clonado? no se escribira", INK_BAD);
        }
    });

    // * Cuantas VERSIONES mas caben. Es lo que de verdad contesta "cuando
    // hara falta el recolector?" -- un porcentaje no lo dice, y la respuesta
    // con 414 GiB son millones.
    row("caben", &mut ty, &|x, y| {
        let free = bloques.saturating_sub(used_count);
        let per_obj = (20 * 1024u64).div_ceil(tam).max(1);
        let mut b = [0u8; 10];
        let n = decimal(free / per_obj, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], INK);
        p.texto(x, y, "  objetos mas de 20 KiB", INK_DIM);
    });

    ty += 8;
    // == LA VERDAD SOBRE LA ESCRITURA, y ahora la bandera SI la dice =========
    //
    // ** HASTA EL 2026-08-18 ESTE `if` ERA CODIGO MUERTO, y la rama de abajo la
    // unica que se veia.
    //
    // `INFO_ES_ESCRIBIBLE` contestaba **un cero constante** en el kernel, con un
    // comentario que decia que la transaccion existia pero que nadie la habia
    // cableado al dispositivo. Era cierto el dia que se escribio; dejo de serlo
    // cuando `sellar` empezo a escribir el superbloque de verdad -- y el disco
    // de esta casa va por la generacion 3, o sea que ha commiteado tres veces.
    //
    // Este panel ya se habia arreglado una vez por exactamente lo mismo, y el
    // arreglo fue prosa: se cambio lo que la rama DICE. El defecto no estaba
    // aqui -- estaba en que el campo no podia decir otra cosa.
    //
    // > Un valor fijo puesto por prudencia envejece hacia la MENTIRA, y no
    // > avisa: lo unico que cambia a su alrededor es el mundo.
    //
    // Ahora la bandera es la conjuncion de las condiciones que de verdad
    // deciden --hay volumen, es de este disco, cabe, y el gate armo la
    // escritura-- asi que las dos ramas significan algo.
    if bmo::info(bmo::INFO_ES_ESCRIBIBLE) != 0 {
        p.texto(tx, ty, "escritura: ABIERTA", INK_OK);
        ty += bmo::GLIFO_ALTO + 3;
        p.texto(tx, ty, "  sellar cierra un estrato y sube la generacion,", INK_DIM);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  con FLUSH CACHE de verdad.  TAB -> S.", INK_DIM);
    } else {
        // ** UN "NO" QUE NO DICE CUAL DE LAS CUATRO ES UN "NO" QUE NO SIRVE.
        //
        // La bandera es una Y de varias condiciones, y cada una manda a mirar
        // un sitio distinto: no hay volumen (se formatea), es de otro disco (se
        // clono), no cabe (hay que recoger), o el gate del disco no armo (eso
        // es del arranque, no de ESTRATOS). Ensenar solo "NO" obligaria a
        // adivinar entre cuatro -- que es lo que costo una vuelta al metal en
        // el recorte del 17-08.
        //
        // Y no hace falta un campo nuevo: las tres primeras ya se preguntan por
        // separado en esta misma ventana, asi que si las tres dicen que si, el
        // que queda es el gate.
        p.texto(tx, ty, "escritura: CERRADA", 0x00F0_D070);
        ty += bmo::GLIFO_ALTO + 3;
        let montado = bmo::info(bmo::INFO_ES_MONTADO) != 0;
        let mio = bmo::info(bmo::INFO_ES_IDENTIDAD) != 0;
        let cabe = bmo::info(bmo::INFO_ES_NIVEL) < 3;
        let porque: &str = if !montado {
            "  no hay volumen montado: se formatea con estratos-fmt."
        } else if !mio {
            "  el volumen NO nacio en este disco: no se le escribe."
        } else if !cabe {
            "  por encima del 95%: solo lectura hasta que se recoja."
        } else {
            "  el gate de identidad del disco no armo la escritura."
        };
        p.texto(tx, ty, porque, INK_DIM);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  sin esto, sellar no escribe y el recorte tampoco.", INK_DIM);
    }

    ty += bmo::GLIFO_ALTO + 10;
    p.texto(tx, ty, "F12 o ESC cierran.   TAB: el explorador.   Ctrl+n: su consola.", INK_DIM);
    ty += bmo::GLIFO_ALTO + 2;
    // * Decirlo aqui evita el susto: con esta ventana delante el teclado es
    // SUYO, asi que teclear no escribe en la caja de abajo. Antes si escribia
    // --en una ventana tapada, sin verlo--, y eso era el fallo.
    p.texto(tx, ty, "mientras este abierta, el teclado es de esta ventana.", INK_DIM);
}

/// Un numero de bytes con su unidad. Devuelve la x donde acabo.
///
/// Sin coma flotante: la parte fraccionaria sale de multiplicar el resto por
/// cien antes de dividir. Es la misma cuenta que hace el panel del kernel, y
/// esta aqui duplicada a proposito -- cruzar el anillo para formatear un numero
/// seria exactamente lo que un library OS no hace.
fn magnitude(p: &bmo::Pantalla, x: u32, y: u32, bytes: u64, color: u32) -> u32 {
    const K: u64 = 1024;
    const M: u64 = K * 1024;
    const G: u64 = M * 1024;
    let (unit, div) = if bytes >= G {
        ("GiB", G)
    } else if bytes >= M {
        ("MiB", M)
    } else if bytes >= K {
        ("KiB", K)
    } else {
        ("B", 1)
    };
    let mut b = [0u8; 10];
    let n = decimal(bytes / div, &mut b);
    let mut x = p.texto_bytes(x, y, &b[..n], color);
    if div > 1 {
        let frac = (bytes % div) * 100 / div;
        x = p.texto(x, y, ".", color);
        if frac < 10 {
            x = p.texto(x, y, "0", color);
        }
        let n = decimal(frac, &mut b);
        x = p.texto_bytes(x, y, &b[..n], color);
    }
    x = p.texto(x, y, " ", color);
    p.texto(x, y, unit, color)
}
