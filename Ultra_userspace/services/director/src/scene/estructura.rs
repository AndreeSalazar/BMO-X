//! **ESTRUCTURA** -- F1, el taller. Escalon 1: la ventana, y nada mas.
//!
//! === Que es esto, y que NO es todavia ===
//!
//! `docs/plan/PLAN_ESTRUCTURA.md` describe un terminal que ademas compila. Esto
//! es su **escalon 1**, y el plan lo dice con esas palabras: *"sin compilador y
//! sin terminal: solo que la tecla llegue"*. Lo que hay aqui es la ventana, su
//! cromo, y un cuerpo que **confiesa en que escalon esta**.
//!
//! ** Y esa confesion no es decoracion. Una ventana vacia y una ventana rota se
//! ven igual, y la unica diferencia esta en si el que mira sabe cual de las dos
//! tiene delante. Por eso el cuerpo dice el escalon y lo que falta: el dia que
//! esto se quede atras, la pantalla lo delata sola.
//!
//! === Por que F1 ===
//!
//! Estaba libre, y no por casualidad: `desktop/keys/app.rs` declaraba
//! `SC_F1 = 0x3B` **solo** como frontera del rango que el escritorio retiene
//! (`SC_F1..=SC_F10`), sin que nadie la usara. Una tecla de funcion no produce
//! caracter en ninguna distribucion, asi que no choca con escribir -- el mismo
//! motivo que F10, F11 y F12.
//!
//! Y va la PRIMERA porque el taller es donde se empieza, no un panel al que se
//! llega. Las de instrumentos --CPU, memoria, sonido, CABINA, datos-- se quedan
//! arriba, en el tramo F7..F12.
//!
//! === El color, que dice cual es antes de leer el titulo ===
//!
//! Azul para el kernel, verde para ESTRATOS, ambar para el sonido. **Violeta
//! para el taller**, y es el mismo del sello de V-ABI que imprime el build --
//! porque ESTRUCTURA es la herramienta que apunta a ese estandar. Ver
//! `VALKYRIE-ABI/README.md`.
//!
//! [!] **ESTRUCTURA no es VALKYRIE, y el color no debe hacer creer que si.**
//! V-ABI juzga y no ejecuta jamas; esto ejecuta en Ring 3. Un dia seran dos
//! procesos distintos y el parecido tiene que quedarse en el color.

use bmo_userland as bmo;

use super::chrome::Chrome;
use super::*;

// Proporcion de la pantalla y no un tamano fijo, como las demas ventanas: ver
// `docs/identidad/LIDERES.md`. Los minimos existen para que el raton no la
// pueda dejar inservible -- por debajo de este ancho el cuerpo no cabe en una
// linea y se corta a media palabra, que es peor que no verlo.
const EST_PCT_W: u32 = 52;
const EST_PCT_H: u32 = 44;
const EST_MIN_W: u32 = 640;
const EST_MIN_H: u32 = 320;

const EST_BG: u32 = 0x0014_0E1A;
const EST_TITLE_BG: u32 = 0x0020_1630;
const EST_EDGE: u32 = 0x0044_2E60;
const EST_TITLE: u32 = 0x00C0_7FD8;

/// La ventana del taller. **Movible**, como todas.
///
/// Solo trae el cromo: no hay estado que recordar todavia. Cuando llegue el
/// escalon 4 --el historial-- ese estado vive en `main.rs` como el del klog y
/// el del sonido, y no aqui: un modulo que pinta y ademas recuerda acaba
/// teniendo dos verdades sobre lo mismo.
pub(crate) struct EstructuraWindow {
    pub(crate) chrome: Chrome,
}

impl EstructuraWindow {
    pub(crate) fn new(p: &bmo::Pantalla) -> Self {
        Self {
            chrome: Chrome::new(p, EST_PCT_W, EST_PCT_H, EST_MIN_W, EST_MIN_H),
        }
    }
}

/// Pinta la ventana entera.
///
/// No recuerda nada entre llamadas, igual que `scene::sound::paint`.
pub(crate) fn paint(p: &bmo::Pantalla, c: &EstructuraWindow) {
    if c.chrome.minimized {
        return;
    }
    // El cromo lo pinta el Marco: sombra, esquinas, barra y los tres botones.
    // Escribirlo a mano aqui es la forma de que un dia el redondeo de esta
    // ventana no case con el de las otras.
    c.chrome.paint_chrome(p, EST_EDGE, EST_BG, EST_TITLE_BG, EST_TITLE);
    c.chrome.paint_buttons(p, EST_TITLE_BG);

    let tx = c.chrome.x + 16;
    p.rect(tx, c.chrome.y + 9, 8, 8, EST_TITLE);
    let px = p.texto(tx + 16, c.chrome.y + 8, "ESTRUCTURA", INK);
    p.texto(px + 2 * bmo::GLIFO_ANCHO, c.chrome.y + 8, "el taller -- F1", INK_DIM);

    let mut ty = c.chrome.y + TITLE_H + 12;
    let salto = bmo::GLIFO_ALTO + 4;

    // ** EL CUERPO CONFIESA EL ESCALON. Ver la cabecera: una ventana vacia y
    // una rota se ven igual, y lo unico que las separa es que esta lo diga.
    p.texto(tx, ty, "escalon 1 de 7: la ventana abre, y eso es todo.", INK);
    ty += salto + 6;

    p.texto(tx, ty, "lo que falta, en orden:", INK_DIM);
    ty += salto;
    for linea in [
        "  2  bmo-rt gana archivo, paquete, superficie, entrada y scroll",
        "  3  dibujar la rejilla y el cursor",
        "  4  leer teclas por el buzon, con historial",
        "  5  los comandos que la frontera permite",
        "  6  compilar hola.ada, aqui dentro",
        "  7  exportar a ESTRATOS o a FAT32",
    ] {
        p.texto(tx, ty, linea, INK_DIM);
        ty += salto;
    }

    ty += 6;
    p.texto(tx, ty, "plan: docs/plan/PLAN_ESTRUCTURA.md", INK_DIM);
}
