//! **La ventana del SONIDO** -- F10, el aparato y quien lo tiene.
//!
//! === Lo que ensena, y por que ese orden ===
//!
//! Arriba el APARATO, porque es la pregunta que decide todo lo demas: si no hay
//! camino, el volumen y las notas no significan nada. Debajo el volumen, y
//! abajo un teclado para probarlo -- porque un control de sonido que no deja
//! hacer ruido no se puede comprobar, y entonces no se sabe si funciona.
//!
//! === * EL COMPOSITOR RECLAMA EL SONIDO AL ABRIRLA Y LO SUELTA AL CERRARLA ===
//!
//! Y esto es lo interesante de la ventana, mas que lo que pinta.
//!
//! `KIND_AUDIO` es **exclusivo**: un solo proceso lo tiene a la vez. Si el
//! escritorio lo reclamara al arrancar --como hace con la pantalla y la
//! entrada-- **ningun programa lanzado desde el podria volver a sonar jamas**,
//! y el sintoma seria `c/musica.bex` diciendo "lo tiene otro proceso" para
//! siempre.
//!
//! Eso ya paso una vez, con la pantalla: `gui.bex` la reclamaba al arrancar y no
//! la soltaba nunca, asi que `ray.bex` no podia pintar. Costo escribir
//! `PANTALLA_SOLTAR` despues, con el fallo delante.
//!
//! Aqui se hace al reves desde el primer dia: **se toma al abrir y se devuelve
//! al cerrar**. La ventana es un huesped del aparato, no su dueno. Por eso
//! `Sonido::release` existia antes de que hubiera nadie que lo llamara.
//!
//! === Por que F10 ===
//!
//! Una tecla de funcion no produce caracter en ninguna distribucion, asi que no
//! choca con escribir -- el mismo motivo que F11 y F12. Y va pegada a ellas
//! porque es la misma familia: ventanas que se abren con una tecla y se leen.

use bmo_userland as bmo;

use super::chrome::Chrome;
use super::*;
use crate::text::decimal;

// Proporcion de la pantalla y no un tamano fijo, como las demas: ver
// `docs/LIDERES.md`. Los minimos existen para que no se pueda dejar
// inservible con el raton -- el teclado de abajo son siete teclas de 52 px y
// por debajo de eso no se puede tocar.
const SND_PCT_W: u32 = 48;
const SND_PCT_H: u32 = 40;
const SND_MIN_W: u32 = 620;
const SND_MIN_H: u32 = 300;

// El ambar es de esta ventana igual que el azul es del kernel y el verde de
// ESTRATOS: el color dice cual es antes de leer el titulo.
const SND_BG: u32 = 0x0018_1105;
const SND_TITLE_BG: u32 = 0x0026_1A08;
const SND_EDGE: u32 = 0x004A_3418;
const SND_TITLE: u32 = 0x00F0_A860;
const SND_BAR: u32 = 0x00D8_8C3A;
const SND_BAR_GAP: u32 = 0x0032_2410;
const KEY_WHITE: u32 = 0x00C8_C0B0;
const KEY_BLACK: u32 = 0x0020_1C18;
const KEY_DOWN: u32 = 0x00F0_A860;

/// Las siete notas del teclado de abajo, y la letra que las toca.
///
/// Es una octava y no mas: caben siete teclas legibles en el ancho de la
/// ventana, y el objeto de esto es **comprobar que suena**, no tocar una pieza.
/// Para una pieza esta `c/musica.bex`, que es un programa y no una ventana.
pub(crate) const NOTES: [(u8, u32, &str); 7] = [
    (b'z', 262, "DO"),
    (b'x', 294, "RE"),
    (b'c', 330, "MI"),
    (b'v', 349, "FA"),
    (b'b', 392, "SOL"),
    (b'n', 440, "LA"),
    (b'm', 494, "SI"),
];

/// La ventana del sonido. **Movible**, como todas las de `gui.bex`.
///
/// Nacio con una caja fija --cuatro numeros y un `contains`-- y eso ya era una
/// excepcion el dia que se escribio: ESTRATOS llevaba `Chrome` desde antes. Una
/// ventana que no se puede apartar tapa justo lo que uno quiere comparar con
/// ella, y aqui eso duele mas que en otras: el volumen se ajusta MIRANDO otra
/// cosa -- lo que suena.
pub(crate) struct SoundWindow {
    pub(crate) chrome: Chrome,
}

impl SoundWindow {
    pub(crate) fn new(p: &bmo::Pantalla) -> Self {
        Self {
            chrome: Chrome::new(p, SND_PCT_W, SND_PCT_H, SND_MIN_W, SND_MIN_H),
        }
    }
}

/// Escribe `s` en `dst` desde `n`, sin salirse. Igual que en `klog`.
fn place(s: &[u8], dst: &mut [u8], n: &mut usize) {
    for &b in s {
        if *n < dst.len() {
            dst[*n] = b;
            *n += 1;
        }
    }
}

fn num(v: u64, dst: &mut [u8], n: &mut usize) {
    let mut d = [0u8; 10];
    let k = decimal(v, &mut d);
    place(&d[..k], dst, n);
}

/// Pinta la ventana entera.
///
/// `aparatos` es la mascara que contesto el kernel (0 = no se pudo preguntar
/// porque el sonido es de otro), `volumen` el que esta puesto, y `pressed` la
/// nota que se esta tocando ahora mismo, si alguna.
///
/// No recuerda nada entre llamadas **a proposito**: el estado de la sesion vive
/// en `main.rs`, igual que el desplazamiento y el filtro del klog. Un modulo que
/// pinta y ademas recuerda acaba teniendo dos verdades sobre lo mismo.
pub(crate) fn paint(
    p: &bmo::Pantalla,
    c: &SoundWindow,
    mine: bool,
    aparatos: u64,
    volumen: u8,
    pressed: Option<usize>,
) {
    if c.chrome.minimized {
        return;
    }
    // El cromo lo pinta el Marco: sombra, esquinas, barra de titulo y los tres
    // botones. Estaba escrito a mano aqui, que es la forma de que un dia el
    // redondeo de esta ventana no case con el de las otras.
    c.chrome.paint_chrome(p, SND_EDGE, SND_BG, SND_TITLE_BG, SND_TITLE);
    c.chrome.paint_buttons(p, SND_TITLE_BG);

    let tx = c.chrome.x + 16;
    p.rect(tx, c.chrome.y + 9, 8, 8, SND_TITLE);
    let px = p.texto(tx + 16, c.chrome.y + 8, "Sonido", INK);
    p.texto(px + 2 * bmo::GLIFO_ANCHO, c.chrome.y + 8, "KIND_AUDIO", INK_DIM);

    let mut ty = c.chrome.y + TITLE_H + 10;

    // -- 1. EL APARATO -------------------------------------------------
    //
    // Primero porque decide lo demas. Y con la salvedad dicha: un bit puesto
    // significa que hay CAMINO, no que se vaya a oir algo -- el puerto del
    // altavoz existe en todo x86 y el zumbador fisico no.
    if !mine {
        p.texto(tx, ty, "el sonido lo tiene OTRO proceso.", INK);
        ty += bmo::GLIFO_ALTO + 4;
        p.texto(tx, ty, "cierra esta ventana y vuelve a abrirla cuando lo suelte.", INK_DIM);
        return;
    }

    let mut lin = [0u8; 80];
    let mut n = 0usize;
    place(b"aparato   ", &mut lin, &mut n);
    if aparatos & 1 != 0 {
        place(b"altavoz del PC", &mut lin, &mut n);
    } else {
        place(b"ninguno", &mut lin, &mut n);
    }
    if aparatos & 2 != 0 {
        place(b" + HD Audio", &mut lin, &mut n);
    }
    if aparatos & 4 != 0 {
        place(b" + audifono USB", &mut lin, &mut n);
    }
    p.texto_bytes(tx, ty, &lin[..n], INK);
    ty += bmo::GLIFO_ALTO + 2;

    // La salvedad, en tenue y siempre. Sin ella, un altavoz que no suena parece
    // un sistema roto -- y la mitad de las placas modernas no traen zumbador.
    //
    // Con audifono USB delante la frase cambia, porque **ahi el volumen si
    // manda**: es un `SET_CUR` sobre su Feature Unit y el aparato obedece.
    p.texto(
        tx,
        ty,
        if aparatos & 4 != 0 {
            "el volumen manda sobre el audifono USB de verdad"
        } else {
            "(hay camino; que suene depende de la placa)"
        },
        INK_DIM,
    );
    ty += bmo::GLIFO_ALTO + 2;
    if aparatos & 2 == 0 {
        p.texto(tx, ty, "HD Audio: sin driver todavia -- casilla 5.1", INK_DIM);
    }
    ty += bmo::GLIFO_ALTO + 10;

    // -- 2. EL VOLUMEN --------------------------------------------------
    let mut lin = [0u8; 40];
    let mut n = 0usize;
    place(b"volumen   ", &mut lin, &mut n);
    num(volumen as u64, &mut lin, &mut n);
    place(b" / 100", &mut lin, &mut n);
    p.texto_bytes(tx, ty, &lin[..n], INK);
    ty += bmo::GLIFO_ALTO + 6;

    let bar_w = c.chrome.width - 32 - 16;
    p.rect(tx, ty, bar_w, 10, SND_BAR_GAP);
    let is_full = (bar_w * volumen as u32) / 100;
    if is_full > 0 {
        p.rect(tx, ty, is_full, 10, SND_BAR);
    }
    // La marca del escalon: en el altavoz del PC el volumen NO es continuo --
    // son dos modos del temporizador, y el corte esta en 50. Dibujarlo evita
    // que parezca que la barra no hace nada entre 51 y 100.
    p.rect(tx + bar_w / 2, ty - 3, 1, 16, SND_TITLE);
    ty += 16;
    p.texto(
        tx,
        ty,
        "el altavoz del PC tiene DOS escalones, no cien: el corte es la marca",
        INK_DIM,
    );
    ty += bmo::GLIFO_ALTO + 12;

    // -- 3. EL TECLADO --------------------------------------------------
    //
    // Siete teclas. Existe para poder COMPROBAR: un control de sonido que no
    // deja hacer ruido no se puede probar, y entonces no se sabe si funciona.
    let key_w = 52u32;
    let key_h = 54u32;
    let sep = 4u32;
    let kx = tx;
    for (i, (letter, _, name)) in NOTES.iter().enumerate() {
        let x = kx + i as u32 * (key_w + sep);
        let color = if pressed == Some(i) { KEY_DOWN } else { KEY_WHITE };
        p.rect(x, ty, key_w, key_h, color);
        // El nombre de la nota arriba y la letra que la toca abajo: la ventana
        // se explica sola sin una leyenda aparte.
        p.texto(x + 8, ty + 8, name, KEY_BLACK);
        let mut l = [0u8; 1];
        l[0] = (*letter as char).to_ascii_uppercase() as u8;
        p.texto_bytes(x + 8, ty + key_h - bmo::GLIFO_ALTO - 8, &l, KEY_BLACK);
    }
    ty += key_h + 8;

    // -- 4. La barra de atajos, del mismo estilo que las otras ventanas --
    p.texto(
        tx,
        ty,
        "flechas volumen   Z..M notas   P la frase   arrastra el titulo   ESC devuelve el aparato",
        INK_DIM,
    );
}
