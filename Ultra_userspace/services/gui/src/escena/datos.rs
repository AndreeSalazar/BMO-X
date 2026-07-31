//! **La consola de DATOS** — F12, el centro de control de ESTRATOS.
//!
//! ═══ Por qué una ventana aparte y no otro comando ═══
//!
//! La caja de `Ejecutar` es de una línea: escribes una ruta y algo corre. Eso
//! sirve para lanzar; no sirve para **mirar un almacén**. Un volumen tiene
//! estado —generación, ocupación, identidad, nivel— que se lee de un vistazo o
//! no se lee, y ponerlo detrás de un comando obliga a teclearlo cada vez que
//! quieres saber si algo cambió.
//!
//! ═══ Por qué F12 y no un Ctrl+algo ═══
//!
//! ★ **Una tecla de función no produce carácter en NINGUNA distribución.** No
//! puede chocar con escribir, y eso es lo único que importa en un atajo del
//! sistema. `Ctrl+Alt` ya lo tiene la ventana de Ejecutar **y es AltGr en
//! español** —lo que da `@ # [ ] \ | €`—, así que ese atajo lleva una danza
//! entera para no romper el teclado: dispara al SOLTAR, y sólo si no llegó
//! ningún carácter mientras estaban pulsados. Encadenar otro combo encima
//! empeoraría justo lo que costó arreglar.
//!
//! Hasta hoy las teclas de función llegaban al kernel y morían ahí: la
//! distribución no las resolvía a ningún byte. El hueco estaba limpio.
//!
//! ═══ Lo que ENSEÑA, y lo que todavía no hace ═══
//!
//! Enseña. Y dice, en alto, que todavía no escribe.
//!
//! La máquina de estados de la transacción existe y está probada
//! (`bmo_estratos::escritura`, 12 tests), pero **nadie la ha cableado al
//! dispositivo**: no hay `write` ni `FLUSH CACHE`. Esta ventana lo pone en
//! pantalla en vez de ofrecer un botón que no hace nada — en un almacén, una
//! promesa de escritura que no ocurre es como se pierde el trabajo de alguien.

use bmo_userland as bmo;

use super::*;
use crate::texto::decimal;

pub(crate) const DATOS_ANCHO: u32 = 640;
pub(crate) const DATOS_ALTO: u32 = 330;

const DATOS_FONDO: u32 = 0x0012_2418;
const DATOS_BORDE: u32 = 0x0037_C871;
const DATOS_TITULO: u32 = 0x008F_F0B8;

/// Los cuatro niveles de `bmo_estratos::espacio`, con su color.
///
/// El orden es el del ABI (`INFO_ES_NIVEL`), no uno inventado aquí: si
/// divergieran, el panel pintaría en verde un volumen en solo lectura.
fn nivel_texto(n: u64) -> (&'static str, u32) {
    match n {
        0 => ("holgado", TEXTO_BIEN),
        1 => ("AVISO: por encima del 70%", 0x00F0_D070),
        2 => ("FAULT: por encima del 85%", TEXTO_MAL),
        _ => ("SOLO LECTURA: por encima del 95%", TEXTO_MAL),
    }
}

/// Dónde va la ventana, centrada sobre el panel.
pub(crate) struct CajaDatos {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) ancho: u32,
    pub(crate) alto: u32,
}

impl CajaDatos {
    pub(crate) fn nueva(p: &bmo::Pantalla) -> Self {
        let ancho = DATOS_ANCHO.min(p.ancho.saturating_sub(40));
        let alto = DATOS_ALTO.min(p.alto.saturating_sub(40));
        Self {
            x: (p.ancho.saturating_sub(ancho)) / 2,
            y: (p.alto.saturating_sub(alto)) / 2,
            ancho,
            alto,
        }
    }

    /// ¿Este píxel cae dentro? Lo necesita el borrado para saber qué repintar.
    pub(crate) fn contiene(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.ancho && py >= self.y && py < self.y + self.alto
    }
}

/// Pinta la consola de datos entera.
///
/// Se redibuja completa en cada invocación y no por fotograma: los números de
/// un volumen no cambian solos mientras nadie escriba, y repintar 200k píxeles
/// sobre memoria de vídeo sin caché sesenta veces por segundo para enseñar los
/// mismos dígitos es tirar el fotograma.
pub(crate) fn pintar(p: &bmo::Pantalla, c: &CajaDatos) {
    p.rect(c.x, c.y, c.ancho, c.alto, DATOS_BORDE);
    p.rect(c.x + 2, c.y + 2, c.ancho - 4, c.alto - 4, DATOS_FONDO);

    let tx = c.x + 16;
    let mut ty = c.y + 14;
    p.texto(tx, ty, "ESTRATOS // centro de datos", DATOS_TITULO);
    ty += bmo::GLIFO_ALTO + 8;

    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        p.texto(tx, ty, "ningun volumen ESTRATOS montado.", TEXTO_MAL);
        ty += bmo::GLIFO_ALTO + 4;
        p.texto(tx, ty, "se formatea desde el anfitrion con estratos-fmt.", TEXTO_TENUE);
        return;
    }

    let bloques = bmo::info(bmo::INFO_ES_BLOQUES);
    let usados = bmo::info(bmo::INFO_ES_USADOS);
    let tam = bmo::info(bmo::INFO_ES_BLOQUE_TAM).max(1);
    let nivel = bmo::info(bmo::INFO_ES_NIVEL);

    let mut fila = |etiqueta: &str, y: &mut u32, pinta: &dyn Fn(u32, u32)| {
        p.texto(tx, *y, etiqueta, TEXTO_TENUE);
        pinta(tx + 13 * bmo::GLIFO_ANCHO, *y);
        *y += bmo::GLIFO_ALTO + 3;
    };

    fila("generacion", &mut ty, &|x, y| {
        let g = bmo::info(bmo::INFO_ES_GENERACION);
        let mut b = [0u8; 10];
        let n = decimal(g, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], TEXTO);
        p.texto(x, y, "  transacciones desde el formateo", TEXTO_TENUE);
    });

    fila("espacio", &mut ty, &|x, y| {
        let x = magnitud(p, x, y, usados * tam, TEXTO);
        let x = p.texto(x, y, " de ", TEXTO_TENUE);
        let x = magnitud(p, x, y, bloques * tam, TEXTO);
        let pct = if bloques == 0 { 0 } else { usados * 100 / bloques };
        let x = p.texto(x, y, "   ", TEXTO_TENUE);
        let mut b = [0u8; 10];
        let n = decimal(pct, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], TEXTO);
        p.texto(x, y, "%", TEXTO);
    });

    fila("estado", &mut ty, &|x, y| {
        let (t, color) = nivel_texto(nivel);
        p.texto(x, y, t, color);
    });

    fila("identidad", &mut ty, &|x, y| {
        if bmo::info(bmo::INFO_ES_IDENTIDAD) != 0 {
            p.texto(x, y, "nacio en ESTE disco", TEXTO_BIEN);
        } else {
            p.texto(x, y, "NO nacio aqui: clonado? no se escribira", TEXTO_MAL);
        }
    });

    // ★ Cuantas VERSIONES mas caben. Es lo que de verdad contesta "¿cuando
    // hara falta el recolector?" — un porcentaje no lo dice, y la respuesta
    // con 414 GiB son millones.
    fila("caben", &mut ty, &|x, y| {
        let libres = bloques.saturating_sub(usados);
        let por_obj = (20 * 1024u64).div_ceil(tam).max(1);
        let mut b = [0u8; 10];
        let n = decimal(libres / por_obj, &mut b);
        let x = p.texto_bytes(x, y, &b[..n], TEXTO);
        p.texto(x, y, "  objetos mas de 20 KiB", TEXTO_TENUE);
    });

    ty += 8;
    // ── La verdad sobre la escritura ──
    if bmo::info(bmo::INFO_ES_ESCRIBIBLE) != 0 {
        p.texto(tx, ty, "escritura: ABIERTA", TEXTO_BIEN);
    } else {
        p.texto(tx, ty, "escritura: CERRADA", TEXTO_MAL);
        ty += bmo::GLIFO_ALTO + 3;
        p.texto(tx, ty, "  la transaccion existe y esta probada (12 tests),", TEXTO_TENUE);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  pero nadie la ha cableado al dispositivo todavia:", TEXTO_TENUE);
        ty += bmo::GLIFO_ALTO + 2;
        p.texto(tx, ty, "  falta el write y el FLUSH CACHE de verdad.", TEXTO_TENUE);
    }

    ty += bmo::GLIFO_ALTO + 10;
    p.texto(tx, ty, "F12 cierra.   leer: ls / lee en la caja de Ejecutar.", TEXTO_TENUE);
}

/// Un número de bytes con su unidad. Devuelve la x donde acabó.
///
/// Sin coma flotante: la parte fraccionaria sale de multiplicar el resto por
/// cien antes de dividir. Es la misma cuenta que hace el panel del kernel, y
/// está aquí duplicada a propósito — cruzar el anillo para formatear un número
/// sería exactamente lo que un library OS no hace.
fn magnitud(p: &bmo::Pantalla, x: u32, y: u32, bytes: u64, color: u32) -> u32 {
    const K: u64 = 1024;
    const M: u64 = K * 1024;
    const G: u64 = M * 1024;
    let (unidad, div) = if bytes >= G {
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
    p.texto(x, y, unidad, color)
}
