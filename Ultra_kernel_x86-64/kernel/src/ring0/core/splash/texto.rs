//! **EL TEXTO** -- la fuente de 8x16 y las cadenas.
//!
//! [carril]  VERDE     la fuente y las cadenas
//!
//! ## Que hace aqui y no en el lienzo
//!
//! Porque una letra no es un rectangulo. El lienzo contesta *donde caen los
//! pixeles*; esto contesta *que pixeles tiene una A*, y son dos preguntas que
//! cambian por motivos distintos: el lienzo cambia el dia que haya una
//! superficie en RAM o una GPU, y la fuente cambia el dia que `fontgen` genere
//! otros glifos.
//!
//! ## La fuente es de paso fijo, y de eso depende media CABINA
//!
//! Cada caracter mide [`CHAR_W`] x [`CHAR_H`] pase lo que pase, asi que el
//! ancho de una cadena es una multiplicacion y no un recorrido. El panel de
//! arranque y la CABINA colocan columnas contando caracteres; con una fuente
//! proporcional habria que medir cada linea antes de decidir donde empieza la
//! siguiente.
//!
//! [!] Los datos de los glifos NO viven aqui: los genera
//! `toolchain/tools/fontgen` en `core/font16_data.rs`, y por eso el `include!`
//! sube un directorio. Mover esos ficheros obligaria a tocar la ruta que el
//! generador lleva escrita, y una ruta generada que no coincide con la real es
//! un fallo que solo aparece el dia que alguien regenera la fuente.

use super::lienzo::{fill_rect, put_pix, wc_flush};
use bmo_dibujo::Lienzo;

// ?????? Font: 8x16 bitmap, chars 32..126 (space through ~) ??????????????????????????????

pub(crate) const FONT_H: usize   = 16;
pub(crate) const FONT_W: usize   = 8;
pub(crate) const CHAR_W: usize   = 10;  // 2px spacing
pub(crate) const CHAR_H: usize   = 20;  // 4px line spacing

static FONT16: [[u8; 16]; 120] = include!("../font16_data.rs");
/// Bytes Latin-1 de los glifos extra, en el mismo orden en que aparecen en
/// FONT16 a partir del indice 95. Generado junto al font: si crece la tabla
/// del generador crecen los dos archivos y aqui solo cambia el tamano.
static FONT_EXTRA: [u8; 25] = include!("../font16_extra.rs");
/// Cuantos glifos ASCII (32..=126) van primero en FONT16.
const ASCII_GLYPHS: usize = 95;

/// Byte -> indice de glifo. ASCII directo; para el espanol (n~, a-acento, ,
/// ...) se busca el byte Latin-1 en la tabla de extras.
///
/// Latin-1 y no UTF-8 a proposito: en Ring 0 un caracter es UN byte, asi el
/// teclado, la linea del shell y el framebuffer hablan el mismo idioma sin
/// decodificador de por medio.
fn glyph_index(c: u8) -> Option<usize> {
    if (32..=126).contains(&c) {
        return Some(c as usize - 32);
    }
    let mut i = 0;
    while i < FONT_EXTRA.len() {
        if FONT_EXTRA[i] == c { return Some(ASCII_GLYPHS + i); }
        i += 1;
    }
    None
}

// ?????? Text drawing ??????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????

pub(crate) fn draw_char(x: u32, y: u32, c: u8, color: u32) {
    let idx = match glyph_index(c) { Some(i) => i, None => return };
    let glyph = &FONT16[idx];

    // Is the glyph pixel at (col,row) set? Out-of-bounds counts as empty.
    let lit = |col: i32, row: i32| -> bool {
        if col < 0 || col >= FONT_W as i32 || row < 0 || row >= FONT_H as i32 {
            return false;
        }
        glyph[row as usize] & (0x80u8 >> col) != 0
    };

    // NITIDO Y SIMPLE (pedido del usuario, monitor 74 Hz): solo el glifo
    // exacto, a color pleno. Antes habia un pase de "anti-alias" que rellenaba
    // las esquinas concavas con un tono tenue (blend 110) -- eso REDONDEA pero
    // DIFUMINA el texto. Sin ese pase, cada pixel es limpio: letras crujientes.
    for row in 0..FONT_H as i32 {
        for col in 0..FONT_W as i32 {
            if lit(col, row) {
                put_pix(x + col as u32, y + row as u32, color);
            }
        }
    }
}

pub(crate) fn draw_str(x: u32, y: u32, s: &str, color: u32) {
    let mut cx = x;
    for b in s.bytes() {
        draw_char(cx, y, b, color);
        cx += CHAR_W as u32;
    }
    // `draw_char` pinta con `put_pix`, que NO drena el buffer WC. Sin este
    // flush, las letras llegan a VRAM tarde/parciales y dejan estela
    // fantasma (el "ghosting" del log rodante y del prompt). Un solo flush
    // por linea -- barato -- mata el efecto en todos los que dibujan texto.
    wc_flush();
}

pub(crate) fn text_width(s: &str) -> u32 {
    s.len() as u32 * CHAR_W as u32
}

pub(crate) fn text_width_scaled(s: &str, scale: u32) -> u32 {
    s.len() as u32 * CHAR_W as u32 * scale
}

/// Un glifo dibujado a `scale`x (cada pixel = un bloque scalexscale). Sin AA:
/// a escala >=3 los bloques ya leen limpios y con peso.
pub(crate) fn draw_char_scaled(x: u32, y: u32, c: u8, color: u32, scale: u32) {
    let idx = match glyph_index(c) { Some(i) => i, None => return };
    let glyph = &FONT16[idx];
    for row in 0..FONT_H {
        let bits = glyph[row];
        for col in 0..FONT_W {
            if bits & (0x80 >> col) != 0 {
                fill_rect(x + col as u32 * scale, y + row as u32 * scale, scale, scale, color);
            }
        }
    }
}

pub(crate) fn draw_str_scaled(x: u32, y: u32, s: &str, color: u32, scale: u32) {
    let mut cx = x;
    for b in s.bytes() {
        draw_char_scaled(cx, y, b, color, scale);
        cx += CHAR_W as u32 * scale;
    }
}

// ?????? Las mismas letras, sobre un lienzo cualquiera ????????????????????????
//
// ** POR QUE HAY DOS PUERTAS Y NO DOS IMPLEMENTACIONES.
//
// Las de arriba escriben en la pantalla y la ensenan al momento: es lo que
// quieren el panel de arranque, la CABINA y la pantalla de fallo, que pintan una
// linea y ya. Las de aqui escriben **donde se les diga**, que es lo que necesita
// la intro desde que pinta en una superficie en RAM para volcarla de una vez.
//
// La forma de las letras --el indice del glifo y el barrido de bits-- es la
// misma en las dos, y esta escrita una sola vez: `glyph_index` y `FONT16`. Lo
// unico que cambia es a donde va el pixel. Duplicar el barrido para tener las
// dos puertas seria repetir el error que este mismo remodelado vino a arreglar.

/// Un glifo, en el lienzo que sea.
pub(crate) fn glifo_en(l: &mut (impl Lienzo + ?Sized), x: i32, y: i32, c: u8, color: u32, escala: i32) {
    let idx = match glyph_index(c) { Some(i) => i, None => return };
    let glyph = &FONT16[idx];
    let escala = escala.max(1);
    for row in 0..FONT_H {
        let bits = glyph[row];
        for col in 0..FONT_W {
            if bits & (0x80 >> col) != 0 {
                l.rect(
                    x + col as i32 * escala,
                    y + row as i32 * escala,
                    escala,
                    escala,
                    color,
                );
            }
        }
    }
}

/// Una cadena, en el lienzo que sea. `escala = 1` es el tamano normal.
pub(crate) fn cadena_en(l: &mut (impl Lienzo + ?Sized), x: i32, y: i32, s: &str, color: u32, escala: i32) {
    let escala = escala.max(1);
    let mut cx = x;
    for b in s.bytes() {
        glifo_en(l, cx, y, b, color, escala);
        cx += CHAR_W as i32 * escala;
    }
}
