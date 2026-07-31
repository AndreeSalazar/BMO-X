//! La calculadora: sus teclas, su caja y su pintado.
//!
//! Vive aparte porque no es del compositor: es una aplicacion que el compositor
//! aloja. Mezclada con el bucle de fotograma parecia parte del sistema.

use bmo_userland as bmo;

use super::*;

// ── La calculadora ──────────────────────────────────────────────────────
//
// La CARA. El cálculo lo hace `cobol/calcgui.bex`, en COBOL, con decimal
// exacto en centavos. Es la separación que Windows no hace —su calculadora
// lleva el motor dentro de la app— y es la que permite cambiar la una sin
// tocar la otra: mañana el motor puede ser Ada y esto no se entera.

pub(crate) const CALC_COLS: usize = 4;
pub(crate) const CALC_ROWS: usize = 5;
pub(crate) const CALC_BOTON: u32 = 72;
pub(crate) const CALC_HUECO: u32 = 6;
pub(crate) const CALC_FONDO: u32 = 0x0018_2434;
pub(crate) const CALC_TECLA: u32 = 0x002B_3B52;
pub(crate) const CALC_TECLA_OP: u32 = 0x003A_5878;
pub(crate) const CALC_TECLA_IGUAL: u32 = 0x004C_9BE8;

/// Las teclas, en el orden en que se dibujan. `\0` = hueco.
pub(crate) const CALC_TECLAS: [[u8; CALC_COLS]; CALC_ROWS] = [
    [b'C', b'/', b'*', b'-'],
    [b'7', b'8', b'9', b'+'],
    [b'4', b'5', b'6', 0],
    [b'1', b'2', b'3', b'='],
    [b'0', b'.', 0, 0],
];

/// Estado de la calculadora. Los operandos se guardan como TEXTO, no como
/// número: quien sabe de números aquí es el COBOL, y convertir dos veces sólo
/// añade sitios donde perder un decimal.
pub(crate) struct Calc {
    pub(crate) visible: bool,
    /// Lo que se está tecleando ahora.
    pub(crate) entrada: [u8; 20],
    pub(crate) n: usize,
    /// El operando de la izquierda, ya cerrado.
    pub(crate) guardado: [u8; 20],
    pub(crate) guardado_n: usize,
    /// 0 = ninguno; 1..4 = + - * /
    pub(crate) op: u8,
    /// Se lanzó el motor y se espera su respuesta.
    pub(crate) esperando: bool,
}

impl Calc {
    pub(crate) fn nueva() -> Self {
        Self {
            visible: false,
            entrada: [0; 20],
            n: 0,
            guardado: [0; 20],
            guardado_n: 0,
            op: 0,
            esperando: false,
        }
    }

    pub(crate) fn meter(&mut self, c: u8) {
        if self.n < self.entrada.len() {
            self.entrada[self.n] = c;
            self.n += 1;
        }
    }

    pub(crate) fn limpiar(&mut self) {
        self.n = 0;
        self.guardado_n = 0;
        self.op = 0;
        self.esperando = false;
    }

    /// Cierra el operando de la izquierda y anota qué operación viene.
    pub(crate) fn operador(&mut self, op: u8) {
        if self.n > 0 {
            self.guardado[..self.n].copy_from_slice(&self.entrada[..self.n]);
            self.guardado_n = self.n;
            self.n = 0;
        }
        self.op = op;
    }

    /// Lo que se enseña en la pantallita: lo que se teclea, o `0` si no hay
    /// nada — una calculadora en blanco confunde.
    pub(crate) fn mostrado(&self) -> &[u8] {
        if self.n == 0 {
            b"0"
        } else {
            &self.entrada[..self.n]
        }
    }
}

/// Geometría del panel, a la derecha de la caja.
pub(crate) struct CalcCaja {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) ancho: u32,
    pub(crate) alto: u32,
    pub(crate) pantalla_y: u32,
    pub(crate) rejilla_y: u32,
}

impl CalcCaja {
    pub(crate) fn nueva(c: &Caja) -> Self {
        let ancho = CALC_COLS as u32 * (CALC_BOTON + CALC_HUECO) + CALC_HUECO;
        let alto = CALC_ROWS as u32 * (CALC_BOTON + CALC_HUECO) + CALC_HUECO + 56;
        Self {
            x: c.x + CAJA_ANCHO + 24,
            y: c.y,
            ancho,
            alto,
            pantalla_y: c.y + CALC_HUECO,
            rejilla_y: c.y + CALC_HUECO + 50,
        }
    }

    /// Rectángulo de la tecla `(fila, col)`.
    pub(crate) fn boton(&self, fila: usize, col: usize) -> (u32, u32) {
        (
            self.x + CALC_HUECO + col as u32 * (CALC_BOTON + CALC_HUECO),
            self.rejilla_y + fila as u32 * (CALC_BOTON + CALC_HUECO),
        )
    }

    /// Qué tecla hay bajo `(px, py)`, si hay alguna.
    pub(crate) fn tecla_en(&self, px: u32, py: u32) -> Option<u8> {
        for fila in 0..CALC_ROWS {
            for col in 0..CALC_COLS {
                let t = CALC_TECLAS[fila][col];
                if t == 0 {
                    continue;
                }
                let (bx, by) = self.boton(fila, col);
                if px >= bx && px < bx + CALC_BOTON && py >= by && py < by + CALC_BOTON {
                    return Some(t);
                }
            }
        }
        None
    }

    pub(crate) fn contiene(&self, px: u32, py: u32) -> bool {
        px >= self.x && px < self.x + self.ancho && py >= self.y && py < self.y + self.alto
    }
}

pub(crate) fn pintar_calc(p: &bmo::Pantalla, cc: &CalcCaja, c: &Calc) {
    p.rect(cc.x, cc.y, cc.ancho, cc.alto, CAJA_BORDE);
    p.rect(cc.x + 2, cc.y + 2, cc.ancho - 4, cc.alto - 4, CALC_FONDO);

    // La pantallita, alineada a la DERECHA como cualquier calculadora: los
    // números se comparan por la unidad, no por la primera cifra.
    p.rect(cc.x + CALC_HUECO, cc.pantalla_y, cc.ancho - CALC_HUECO * 2, 40, CAMPO_FONDO);
    let texto = c.mostrado();
    let ancho_texto = texto.len() as u32 * bmo::GLIFO_ANCHO;
    let tx = cc.x + cc.ancho - CALC_HUECO - 8 - ancho_texto;
    p.texto_bytes(tx, cc.pantalla_y + 12, texto, if c.esperando { TEXTO_TENUE } else { TEXTO });

    for fila in 0..CALC_ROWS {
        for col in 0..CALC_COLS {
            let t = CALC_TECLAS[fila][col];
            if t == 0 {
                continue;
            }
            let (bx, by) = cc.boton(fila, col);
            let color = match t {
                b'=' => CALC_TECLA_IGUAL,
                b'+' | b'-' | b'*' | b'/' | b'C' => CALC_TECLA_OP,
                _ => CALC_TECLA,
            };
            p.rect(bx, by, CALC_BOTON, CALC_BOTON, color);
            // La etiqueta, centrada.
            p.glifo(
                bx + CALC_BOTON / 2 - bmo::GLIFO_ANCHO / 2,
                by + CALC_BOTON / 2 - bmo::GLIFO_ALTO / 2,
                t,
                TEXTO,
            );
        }
    }
}

