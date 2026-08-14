//! La calculadora: sus teclas, su caja y su pintado.
//!
//! Vive aparte porque no es del compositor: es una aplicacion que el compositor
//! aloja. Mezclada con el bucle de fotograma parecia parte del sistema.

use bmo_userland as bmo;

use super::*;

// -- La calculadora ------------------------------------------------------
//
// La CARA. El calculo lo hace `cobol/calcgui.bex`, en COBOL, con decimal
// exacto en centavos. Es la separacion que Windows no hace --su calculadora
// lleva el motor dentro de la app-- y es la que permite cambiar la una sin
// tocar la otra: manana el motor puede ser Ada y esto no se entera.

pub(crate) const CALC_COLS: usize = 4;
pub(crate) const CALC_ROWS: usize = 5;
pub(crate) const CALC_BTN: u32 = 72;
pub(crate) const CALC_GAP: u32 = 6;
pub(crate) const CALC_BG: u32 = 0x0018_2434;
pub(crate) const CALC_KEY: u32 = 0x002B_3B52;
pub(crate) const CALC_KEY_OP: u32 = 0x003A_5878;
pub(crate) const CALC_KEY_EQ: u32 = 0x004C_9BE8;

/// Las teclas, en el orden en que se dibujan. `\0` = hueco.
pub(crate) const CALC_KEYS: [[u8; CALC_COLS]; CALC_ROWS] = [
    [b'C', b'/', b'*', b'-'],
    [b'7', b'8', b'9', b'+'],
    [b'4', b'5', b'6', 0],
    [b'1', b'2', b'3', b'='],
    [b'0', b'.', 0, 0],
];

/// Estado de la calculadora. Los operandos se guardan como TEXTO, no como
/// numero: quien sabe de numeros aqui es el COBOL, y convertir dos veces solo
/// anade sitios donde perder un decimal.
pub(crate) struct Calc {
    pub(crate) visible: bool,
    /// Lo que se esta tecleando ahora.
    pub(crate) input: [u8; 20],
    pub(crate) n: usize,
    /// El operando de la izquierda, ya cerrado.
    pub(crate) saved_path: [u8; 20],
    pub(crate) saved_n: usize,
    /// 0 = ninguno; 1..4 = + - * /
    pub(crate) op: u8,
    /// Se lanzo el motor y se espera su respuesta.
    pub(crate) waiting: bool,
}

impl Calc {
    pub(crate) fn new() -> Self {
        Self {
            visible: false,
            input: [0; 20],
            n: 0,
            saved_path: [0; 20],
            saved_n: 0,
            op: 0,
            waiting: false,
        }
    }

    pub(crate) fn feed(&mut self, c: u8) {
        if self.n < self.input.len() {
            self.input[self.n] = c;
            self.n += 1;
        }
    }

    pub(crate) fn clear(&mut self) {
        self.n = 0;
        self.saved_n = 0;
        self.op = 0;
        self.waiting = false;
    }

    /// Cierra el operando de la izquierda y anota que operacion viene.
    pub(crate) fn operator(&mut self, op: u8) {
        if self.n > 0 {
            self.saved_path[..self.n].copy_from_slice(&self.input[..self.n]);
            self.saved_n = self.n;
            self.n = 0;
        }
        self.op = op;
    }

    /// Lo que se ensena en la pantallita: lo que se teclea, o `0` si no hay
    /// nada -- una calculadora en blanco confunde.
    pub(crate) fn shown(&self) -> &[u8] {
        if self.n == 0 {
            b"0"
        } else {
            &self.input[..self.n]
        }
    }
}

/// Geometria del panel, a la derecha de la caja.
pub(crate) struct CalcPad {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) screen_y: u32,
    pub(crate) rejilla_y: u32,
}

impl CalcPad {
    pub(crate) fn new(c: &RunBox) -> Self {
        let width = CALC_COLS as u32 * (CALC_BTN + CALC_GAP) + CALC_GAP;
        let height = CALC_ROWS as u32 * (CALC_BTN + CALC_GAP) + CALC_GAP + 56;
        Self {
            x: c.x + BOX_W + 24,
            y: c.y,
            width,
            height,
            screen_y: c.y + CALC_GAP,
            rejilla_y: c.y + CALC_GAP + 50,
        }
    }

    /// Rectangulo de la tecla `(row, col)`.
    pub(crate) fn button(&self, row: usize, col: usize) -> (u32, u32) {
        (
            self.x + CALC_GAP + col as u32 * (CALC_BTN + CALC_GAP),
            self.rejilla_y + row as u32 * (CALC_BTN + CALC_GAP),
        )
    }

    /// Que tecla hay bajo `(px, py)`, si hay alguna.
    pub(crate) fn key_at(&self, px: u32, py: u32) -> Option<u8> {
        for row in 0..CALC_ROWS {
            for col in 0..CALC_COLS {
                let t = CALC_KEYS[row][col];
                if t == 0 {
                    continue;
                }
                let (bx, by) = self.button(row, col);
                if px >= bx && px < bx + CALC_BTN && py >= by && py < by + CALC_BTN {
                    return Some(t);
                }
            }
        }
        None
    }

    pub(crate) fn contains(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// Cuanto se aclara un boton cuando el puntero esta encima.
///
/// * El realce va por SUMA y no por un color aparte, y es a proposito: cada
/// clase de tecla tiene el suyo --igual, operador, digito-- y una tabla de
/// colores "de encima" seria el doble de constantes que mantener sincronizadas.
/// Sumar conserva la familia del boton y solo dice "este".
const HIGHLIGHT: u32 = 0x0020_2830;

fn lighten(color: u32) -> u32 {
    // Canal a canal y con tope: sumar sobre el `u32` entero desbordaria de un
    // componente al siguiente y un boton azul se volveria verde al pasar por
    // encima.
    let r = ((color >> 16) & 0xFF).min(0xFF - ((HIGHLIGHT >> 16) & 0xFF)) + ((HIGHLIGHT >> 16) & 0xFF);
    let g = ((color >> 8) & 0xFF).min(0xFF - ((HIGHLIGHT >> 8) & 0xFF)) + ((HIGHLIGHT >> 8) & 0xFF);
    let b = (color & 0xFF).min(0xFF - (HIGHLIGHT & 0xFF)) + (HIGHLIGHT & 0xFF);
    (r << 16) | (g << 8) | b
}

/// Pinta la calculadora. `hover` es la tecla que tiene el puntero encima.
pub(crate) fn paint_calc(p: &bmo::Pantalla, cc: &CalcPad, c: &Calc, hover: Option<u8>) {
    p.rect(cc.x, cc.y, cc.width, cc.height, BOX_EDGE);
    p.rect(cc.x + 2, cc.y + 2, cc.width - 4, cc.height - 4, CALC_BG);

    // La pantallita, alineada a la DERECHA como cualquier calculadora: los
    // numeros se comparan por la unidad, no por la primera cifra.
    p.rect(cc.x + CALC_GAP, cc.screen_y, cc.width - CALC_GAP * 2, 40, FIELD_BG);
    let text = c.shown();
    let text_w = text.len() as u32 * bmo::GLIFO_ANCHO;
    let tx = cc.x + cc.width - CALC_GAP - 8 - text_w;
    p.texto_bytes(tx, cc.screen_y + 12, text, if c.waiting { INK_DIM } else { INK });

    for row in 0..CALC_ROWS {
        for col in 0..CALC_COLS {
            let t = CALC_KEYS[row][col];
            if t == 0 {
                continue;
            }
            let (bx, by) = cc.button(row, col);
            let base = match t {
                b'=' => CALC_KEY_EQ,
                b'+' | b'-' | b'*' | b'/' | b'C' => CALC_KEY_OP,
                _ => CALC_KEY,
            };
            // El boton bajo el puntero se aclara. Es la otra mitad de la mano:
            // el cursor dice "aqui se pulsa" y esto dice "esto de aqui".
            let color = if hover == Some(t) { lighten(base) } else { base };
            p.rect(bx, by, CALC_BTN, CALC_BTN, color);
            // La etiqueta, centrada.
            p.glifo(
                bx + CALC_BTN / 2 - bmo::GLIFO_ANCHO / 2,
                by + CALC_BTN / 2 - bmo::GLIFO_ALTO / 2,
                t,
                INK,
            );
        }
    }
}

