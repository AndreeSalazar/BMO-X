//! La calculadora: su estado. **Su cara la compila MAQUETA.**
//!
//! Vive aparte porque no es del compositor: es una aplicacion que el compositor
//! aloja. Mezclada con el bucle de fotograma parecia parte del sistema.
//!
//! ## ** QUE SE FUE DE ESTE FICHERO, Y ADONDE
//!
//! ```text
//!    CALC_COLS/ROWS/BTN/GAP, los cuatro colores, CALC_KEYS
//!    CalcPad::new, button(), key_at(), contains()
//!    paint_calc(), HIGHLIGHT, lighten()
//!         |
//!         v
//!    toolchain/tools/maqueta/pruebas/calc.maqueta   <- lo que se EDITA
//!    scene/calc_gen.rs                              <- generado, no se toca
//! ```
//!
//! Lo que se quedo es la MAQUINA DE ESTADOS, y se queda para siempre: lo que se
//! teclea, el operando cerrado, que operacion viene. MAQUETA compila lo que esta
//! quieto; esto es lo que cambia.
//!
//! ## El reparto entero, en tres piezas
//!
//! ```text
//!    MAQUETA   la CARA     rects, etiquetas y la tabla de golpeo
//!    Rust      la MANO     este fichero: el estado y quien lanza el motor
//!    COBOL     la CABEZA   cobol/calcgui.bex, decimal exacto en centavos
//! ```
//!
//! Es la separacion que Windows no hace --su calculadora lleva el motor dentro--
//! y es la que permite cambiar una sin tocar las otras: manana el motor puede ser
//! Ada y ni esto ni el `.maqueta` se enteran.
//!
//! [!] Para regenerar la cara:
//!
//! ```text
//!   cargo run -p bmo-maqueta -- toolchain/tools/maqueta/pruebas/calc.maqueta //!                               Ultra_userspace/services/gui/src/scene/calc_gen.rs
//! ```

use bmo_userland as bmo;

use super::calc_gen;
use super::*;

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
    /// **De quien son las teclas.** Con la calculadora abierta las cifras
    /// significan dos cosas --el comando que se escribe y el operando--, asi
    /// que hay que DECIRLO y no adivinarlo. `Ctrl+n` lo cambia; el porque
    /// entero esta en `desktop::calc`.
    pub(crate) keys: bool,
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
            keys: false,
        }
    }

    pub(crate) fn feed(&mut self, c: u8) {
        if self.n < self.input.len() {
            self.input[self.n] = c;
            self.n += 1;
        }
    }

    /// Borrar la ultima cifra tecleada.
    ///
    /// **No hay boton para esto en la cara** --el `.maqueta` no tiene tecla de
    /// retroceso-- y por eso vive aqui y no en la tabla de teclas: es una
    /// afordancia del teclado, no un dibujo. Con `C` al lado, una calculadora
    /// de raton no lo necesita; escribiendo, equivocarse de una cifra y perder
    /// el numero entero es lo que hace que se deje de usar.
    pub(crate) fn backspace(&mut self) {
        if self.n > 0 {
            self.n -= 1;
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

/// Donde esta el panel. **Ya no calcula nada**: la geometria entera vive en
/// `calc_gen`, y aqui solo queda de que esquina cuelga.
///
/// ** Antes esta struct tenia `screen_y` y `rejilla_y`, y `button(row, col)`
/// rehacia la rejilla. La misma aritmetica estaba ademas en `key_at`, una vez
/// para pintar y otra para responder al raton -- y esa duplicacion era una clase
/// de bug entera. Ahora las dos salen de la misma pasada del compilador.
pub(crate) struct CalcPad {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl CalcPad {
    pub(crate) fn new(c: &RunBox) -> Self {
        Self {
            // Al lado de la terminal, sabiendo lo que MIDE ahora y no lo que
            // media cuando estaba clavada: desde que la ventana se estira,
            // `BOX_W` es su minimo y no su ancho.
            x: c.x + c.w() + 24,
            y: c.y,
            width: calc_gen::ANCHO,
            height: calc_gen::ALTO,
        }
    }

    /// Que tecla hay bajo `(px, py)`, si hay alguna.
    ///
    /// El compilador devuelve el `id` del `.maqueta` (`k_7`, `k_add`); aqui se
    /// traduce al byte que espera la maquina de estados. **Esa traduccion es lo
    /// unico que queda de la vieja tabla `CALC_KEYS`**, y es conducta, no
    /// maquetacion: MAQUETA no sabe ni tiene por que saber que `k_add` suma.
    pub(crate) fn key_at(&self, px: u32, py: u32) -> Option<u8> {
        tecla_de(calc_gen::golpe(self.x, self.y, px, py)?)
    }

    // [!] Aqui habia un `contains()` y **no lo llamaba nadie** -- llevaba muerto
    // desde antes de este puerto. La regla de la casa es cablear o borrar, asi
    // que se borra: si vuelve a hacer falta, el modulo generado ya trae
    // `calc_gen::dentro`, que es la misma pregunta sin escribir la resta.
}

/// `id` del `.maqueta` -> el byte que entiende `Calc`.
fn tecla_de(id: &str) -> Option<u8> {
    Some(match id {
        "k_c" => b'C',
        "k_div" => b'/',
        "k_mul" => b'*',
        "k_sub" => b'-',
        "k_add" => b'+',
        "k_eq" => b'=',
        "k_dot" => b'.',
        "k_0" => b'0',
        "k_1" => b'1',
        "k_2" => b'2',
        "k_3" => b'3',
        "k_4" => b'4',
        "k_5" => b'5',
        "k_6" => b'6',
        "k_7" => b'7',
        "k_8" => b'8',
        "k_9" => b'9',
        _ => return None,
    })
}

/// El otro sentido, para saber que caja realzar cuando el raton esta encima.
fn id_de(tecla: u8) -> Option<&'static str> {
    Some(match tecla {
        b'C' => "k_c",
        b'/' => "k_div",
        b'*' => "k_mul",
        b'-' => "k_sub",
        b'+' => "k_add",
        b'=' => "k_eq",
        b'.' => "k_dot",
        b'0' => "k_0",
        b'1' => "k_1",
        b'2' => "k_2",
        b'3' => "k_3",
        b'4' => "k_4",
        b'5' => "k_5",
        b'6' => "k_6",
        b'7' => "k_7",
        b'8' => "k_8",
        b'9' => "k_9",
        _ => return None,
    })
}

/// Pinta la calculadora. `hover` es la tecla que tiene el puntero encima.
///
/// Tres llamadas donde habia cuarenta lineas de bucles y restas:
///
/// 1. `pintar` -- todo lo quieto, ya desenrollado por el compilador.
/// 2. `realce` -- la tecla de debajo del puntero, con sus colores de `:hover`.
/// 3. la pantallita -- **lo unico que cambia**, y por eso es lo unico que sigue
///    siendo Rust: MAQUETA compila lo que esta quieto.
pub(crate) fn paint_calc(p: &bmo::Pantalla, cc: &CalcPad, c: &Calc, hover: Option<u8>) {
    calc_gen::pintar(p, cc.x, cc.y);

    if let Some(id) = hover.and_then(id_de) {
        calc_gen::realce(p, cc.x, cc.y, id);
    }

    // ** La pantallita es una ISLA, y ahi cae la frontera del proyecto entera:
    // el numero CAMBIA, asi que no es maquetacion. MAQUETA le reserva el sitio y
    // le pone el fondo; quien tiene el numero lo escribe.
    //
    // Ni el rect ni el color estan escritos aqui: los dos se le piden al modulo
    // generado, o serian una segunda verdad que envejece cuando cambie el
    // `.maqueta`.
    let Some((vx, vy, vw, vh)) = calc_gen::isla("visor") else {
        return;
    };
    calc_gen::limpiar_isla(p, cc.x, cc.y, "visor");

    // ** EL CURSOR DICE DE QUIEN SON LAS TECLAS, y hace falta decirlo.
    //
    // La calculadora se pinta pegada a la derecha de Ejecutar, que tiene su
    // propio cursor parpadeando. Con las dos a la vista y sin esta marca, la
    // unica forma de saber donde va a caer un `7` es pulsarlo y mirar -- y una
    // cifra que aparece en el comando equivocado ya ensucio la linea.
    //
    // Ocupa su hueco ANTES de colocar el numero: si no, el cursor se pintaria
    // encima de la ultima cifra en vez de detras de ella.
    let caret = if c.keys { bmo::GLIFO_ANCHO } else { 0 };

    // Alineado a la DERECHA como cualquier calculadora: los numeros se comparan
    // por la unidad, no por la primera cifra.
    let text = c.shown();
    let text_w = text.len() as u32 * bmo::GLIFO_ANCHO;
    let ty = cc.y + vy + (vh - bmo::GLIFO_ALTO) / 2;
    p.texto_bytes(
        cc.x + vx + vw.saturating_sub(8 + caret + text_w),
        ty,
        text,
        if c.waiting { INK_DIM } else { INK },
    );
    if c.keys {
        p.texto_bytes(cc.x + vx + vw.saturating_sub(8 + caret), ty, b"_", INK_DIM);
    }
}
