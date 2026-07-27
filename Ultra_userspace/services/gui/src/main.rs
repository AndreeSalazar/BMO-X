//! **El compositor de BMO.** El proceso Ring 3 que es dueño de la pantalla.
//!
//! ## Estado: primer escritorio vivo, con dos cosas por medir
//!
//! La primera foto en hardware salió con la geometría EXACTA —la barra en su
//! sitio, la marca sin escalonar, o sea que el stride es el que dijo el
//! kernel— pero con los colores mucho más claros de lo que dice el código.
//! `0x141C2B` es un azul casi negro y en pantalla salió un azul medio.
//!
//! Adivinar el orden de canales mirando la foto de una pantalla es exactamente
//! la clase de cosa que se cobra una tarde. Así que este compositor pinta una
//! **tira de medida**: seis parches de valores conocidos y puros. Una foto y
//! se acabó la discusión —
//!
//! - si el parche `0x00FF0000` sale ROJO, el formato es XRGB como creemos;
//! - si sale AZUL, los canales están al revés (BGR) y hay que voltearlos;
//! - si el parche `0x00202020` sale gris medio en vez de casi negro, entonces
//!   no es orden de canales: algo está tocando la intensidad (el panel, o el
//!   propio formato del GOP).
//!
//! ## El puntero
//!
//! El kernel lee el HID y entrega coordenadas por `KIND_INPUT`. **El cursor lo
//! dibuja este proceso**, porque su forma y su color son decisiones de aspecto
//! y ninguna de ésas tiene nada que hacer en Ring 0.
//!
//! Repintar por daño, no la pantalla entera: en cada vuelta se restaura sólo
//! el rectángulo que ocupaba el cursor —recalculando de la escena qué había
//! debajo— y se dibuja en la posición nueva. Es como funciona un compositor de
//! verdad, y aquí además es obligatorio: llenar 8 MB por fotograma sobre
//! memoria de vídeo sin caché sería un pase de diapositivas.

#![no_std]
#![no_main]

use bmo_userland as bmo;

// ── La escena ───────────────────────────────────────────────────────────

const FONDO: u32 = 0x0014_1C2B;
const BARRA: u32 = 0x0028_3448;
const ACENTO: u32 = 0x004C_9BE8;

const BARRA_ALTO: u32 = 44;

/// Los seis parches de medida, con sus valores EXACTOS. No son decorativos:
/// cada uno responde una pregunta distinta sobre el formato.
const MEDIDA: [u32; 6] = [
    0x00FF_0000, // ¿rojo o azul? -> orden de canales
    0x0000_FF00, // verde: el canal de en medio no cambia con el orden
    0x0000_00FF, // el complementario del primero
    0x00FF_FFFF, // blanco: el techo
    0x0080_8080, // gris medio: la mitad
    0x0020_2020, // casi negro: si esto sale claro, no es orden, es intensidad
];
const MEDIDA_LADO: u32 = 56;
const MEDIDA_Y: u32 = BARRA_ALTO + 24;
const MEDIDA_X: u32 = 24;

/// Qué color le toca a un píxel según la escena. Es el modelo entero del
/// escritorio, y es lo que permite borrar el cursor sin repintarlo todo:
/// para restaurar una zona basta con volver a preguntar qué había ahí.
fn color_escena(x: u32, y: u32) -> u32 {
    if y < BARRA_ALTO {
        // La marca de referencia dentro de la barra.
        if x >= 16 && x < 32 && y >= 14 && y < 30 {
            return ACENTO;
        }
        return BARRA;
    }
    if y >= MEDIDA_Y && y < MEDIDA_Y + MEDIDA_LADO && x >= MEDIDA_X {
        let i = (x - MEDIDA_X) / MEDIDA_LADO;
        if (i as usize) < MEDIDA.len() {
            return MEDIDA[i as usize];
        }
    }
    FONDO
}

// ── El cursor ───────────────────────────────────────────────────────────

const CUR_ANCHO: usize = 10;
const CUR_ALTO: usize = 16;
/// 0 = transparente, 1 = relleno, 2 = borde.
///
/// Borde oscuro alrededor del relleno claro: es lo que hace que una flecha se
/// vea igual de bien sobre un fondo claro que sobre uno oscuro. No es adorno,
/// es la razón de que todos los cursores del mundo tengan contorno.
const FLECHA: [[u8; CUR_ANCHO]; CUR_ALTO] = [
    [2, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 2, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 1, 2, 0, 0, 0, 0, 0, 0, 0],
    [2, 1, 1, 2, 0, 0, 0, 0, 0, 0],
    [2, 1, 1, 1, 2, 0, 0, 0, 0, 0],
    [2, 1, 1, 1, 1, 2, 0, 0, 0, 0],
    [2, 1, 1, 1, 1, 1, 2, 0, 0, 0],
    [2, 1, 1, 1, 1, 1, 1, 2, 0, 0],
    [2, 1, 1, 1, 1, 1, 1, 1, 2, 0],
    [2, 1, 1, 1, 1, 1, 2, 2, 2, 2],
    [2, 1, 1, 2, 1, 1, 2, 0, 0, 0],
    [2, 1, 2, 0, 2, 1, 1, 2, 0, 0],
    [2, 2, 0, 0, 2, 1, 1, 2, 0, 0],
    [2, 0, 0, 0, 0, 2, 1, 1, 2, 0],
    [0, 0, 0, 0, 0, 2, 1, 2, 0, 0],
    [0, 0, 0, 0, 0, 0, 2, 2, 0, 0],
];
const CUR_RELLENO: u32 = 0x00FF_FFFF;
const CUR_BORDE: u32 = 0x0000_0000;

fn dibujar_cursor(p: &bmo::Pantalla, x: u32, y: u32) {
    for (fila, linea) in FLECHA.iter().enumerate() {
        for (col, &v) in linea.iter().enumerate() {
            if v == 0 {
                continue;
            }
            let color = if v == 1 { CUR_RELLENO } else { CUR_BORDE };
            p.punto(x + col as u32, y + fila as u32, color);
        }
    }
}

/// Restaura de la escena el rectángulo donde estaba el cursor.
fn borrar_cursor(p: &bmo::Pantalla, x: u32, y: u32) {
    for fila in 0..CUR_ALTO as u32 {
        for col in 0..CUR_ANCHO as u32 {
            let (px, py) = (x + col, y + fila);
            p.punto(px, py, color_escena(px, py));
        }
    }
}

// ── El programa ─────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // El aviso va ANTES de reclamar: en cuanto la cesión se consuma, el kernel
    // deja de dibujar y nada de lo que se imprima después llega al panel.
    bmo::consola("reclamo pantalla y raton\n");

    let Some(p) = bmo::Pantalla::reclamar() else {
        bmo::consola("sin pantalla que reclamar\n");
        bmo::salir()
    };
    // El ratón es opcional a propósito: sin él hay escritorio, sólo que
    // quieto. Un compositor que se niega a arrancar porque falta un periférico
    // es un compositor que no arranca el día que el periférico falla.
    let raton = bmo::Raton::reclamar();

    // Fondo entero de una pasada, y encima la escena.
    p.limpiar(FONDO);
    p.rect(0, 0, p.ancho, BARRA_ALTO, BARRA);
    p.rect(16, 14, 16, 16, ACENTO);
    let mut i = 0u32;
    while (i as usize) < MEDIDA.len() {
        p.rect(
            MEDIDA_X + i * MEDIDA_LADO,
            MEDIDA_Y,
            MEDIDA_LADO,
            MEDIDA_LADO,
            MEDIDA[i as usize],
        );
        i += 1;
    }

    bmo::consola("escritorio pintado\n");

    // ── El bucle de vida ──
    //
    // No termina: si saliera, `revoke_all` devolvería la pantalla y el kernel
    // repintaría su panel encima. Un escritorio es un proceso que VIVE — y de
    // paso esto ejerce el cambio de contexto miles de veces por segundo, que
    // es justo el camino que costó una foto de madrugada.
    let (mut ax, mut ay) = (u32::MAX, u32::MAX);
    loop {
        if let Some(r) = raton.as_ref() {
            let pos = r.leer();
            if pos.x != ax || pos.y != ay {
                if ax != u32::MAX {
                    borrar_cursor(&p, ax, ay);
                }
                dibujar_cursor(&p, pos.x, pos.y);
                ax = pos.x;
                ay = pos.y;
            }
        }
        bmo::ceder();
    }
}

/// Un pánico aquí no puede tumbar nada más que a este proceso: lo dice y sale
/// por la puerta normal. El kernel revoca sus capabilities —incluidas la
/// pantalla y el ratón— y sigue vivo.
#[panic_handler]
fn panico(_info: &core::panic::PanicInfo) -> ! {
    bmo::consola("panico en el compositor\n");
    bmo::salir()
}
