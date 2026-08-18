//! **What a calculator key does** -- and it is written ONCE.
//!
//! ## Por que existe este fichero
//!
//! Hasta hoy la calculadora era una app **de raton y nada mas**: el unico
//! sitio de todo el escritorio que la alimentaba era `mouse.rs`, y la carpeta
//! `keys/` la nombraba solo para dejarla fuera. Teclear `7 + 3 =` no hacia
//! absolutamente nada.
//!
//! Anadir el teclado copiando ese bloque a `keys/` habria reconstruido el fallo
//! que este mismo aparato acababa de borrar: `button()` y `key_at()` eran la
//! misma aritmetica escrita dos veces, y el lanzamiento del motor habria pasado
//! a estarlo tambien -- con uno de los dos arreglandose el dia que cambie y el
//! otro no. Asi que el bloque SALIO del raton primero, y los dos llamadores
//! preguntan a la misma funcion.
//!
//! ```text
//!    raton     golpe(x,y)          -> tecla -> pulsar()
//!    teclado   tecla_de_teclado(c) -> tecla -> pulsar()
//! ```
//!
//! ## ** QUIEN SE QUEDA LAS TECLAS
//!
//! La misma ley que la consola de ESTRATOS (`scene/consola.rs`) y por el mismo
//! motivo: con la calculadora abierta **las cifras significan dos cosas** --lo
//! que estas escribiendo en la linea de Ejecutar, y el operando--. Dos duenos
//! para una tecla se resuelve con un ORDEN, nunca con una adivinanza.
//!
//! ```text
//!   `calc`        la abre Y le da el teclado
//!   Ctrl+n        se lo devuelve a la linea SIN cerrarla, y se lo vuelve a dar
//!   con teclado   TODA tecla es suya, tambien las que no entiende
//! ```
//!
//! ** `Ctrl+n` es la misma tecla que abre la consola de ESTRATOS, y a
//! proposito: significa lo mismo en las dos ventanas --*dale el teclado al
//! aparato de esta ventana*-- y no chocan, porque cada rama exige SU foco.
//!
//! [!] **Y ESC no sirve aqui, aunque en la consola si.** `windows::on_key`
//! corre antes que esto y se queda el ESC si estan abiertas las vitales o
//! CABINA **sin mirar el foco** (`keys/windows.rs:71`). Un ESC que suelta el
//! teclado solo cuando no hay otra ventana abierta es peor que ninguno: se
//! aprende mal y falla justo el dia que hace falta. Una tecla que a veces no
//! es lo que dice es la clase de mentira que este arbol persigue.

use bmo_userland as bmo;

use super::keys::Key;
use super::{Desktop, Ventana};
use crate::scene::calc::paint_calc;
use crate::scene::{paint_status, INK_BAD};

/// El atajo que PIDE y DEVUELVE el teclado.
///
/// La `n` produce el byte `0xF1` en la distribucion espanola
/// (`ring0/dev/keyboard.rs`) y `MOD_CTRL` llega entero a Ring 3, asi que no
/// choca con AltGr --que en espanol es `Ctrl+Alt`--, la trampa que ya costo una
/// sesion entera de teclado. Ver `scene/consola.rs`.
const PEDIR_TECLADO: u8 = 0xF1;

// -- ** LOS CODIGOS QUE ENTIENDE EL MOTOR --
//
// Son el contrato con `toolchain/lang/cobol/examples/2-decimal/calcgui.cob`, y
// viven aqui con nombre porque un `4` suelto en una llamada no dice "dividir".
// Cambiar uno sin cambiar el otro da una cuenta equivocada y ningun error: el
// motor contestaria lo que le pidieron, que no es lo que se quiso.
//
// [!] No hay codigo para `+/-`: cambiar el signo es EDITAR el operando, no
// calcular con el, y no llega al motor. Ver `Calc::negate`.
const OP_SUMA: u8 = 1;
const OP_RESTA: u8 = 2;
const OP_POR: u8 = 3;
const OP_ENTRE: u8 = 4;
/// Tanto por ciento: el segundo operando por ciento del primero.
const OP_CIENTO: u8 = 5;
/// Presentar como dinero. **Es de un solo operando**: no calcula, formatea.
const OP_DINERO: u8 = 6;

/// La tecla del teclado que se ofrece a la calculadora.
///
/// Devuelve [`Key::Pass`] mientras las teclas no sean suyas, que es lo que deja
/// seguir escribiendo comandos con la calculadora abierta.
pub(crate) fn on_key(dsk: &mut Desktop, p: &bmo::Pantalla, c: u8, ctrl: bool) -> Key {
    // Cerrada, o con el foco en otra ventana, aqui no hay nada que decidir.
    // La guarda del foco es la misma que la de los demas paneles: con el foco
    // en Datos, un `7` es de Datos.
    if !dsk.calc.visible || !dsk.win.focus.es_para(Ventana::Run) {
        return Key::Pass;
    }

    // ** El atajo va ANTES que todo lo demas, porque tiene que poder RECUPERAR
    // las teclas cuando la calculadora ya las solto. Un atajo que solo funciona
    // si ya tienes el foco no sirve para pedir el foco.
    if ctrl && c == PEDIR_TECLADO {
        dsk.calc.keys = !dsk.calc.keys;
        paint_calc(p, &dsk.calc_pad, &dsk.calc, dsk.tick.calc_hover);
        return Key::Taken;
    }
    if !dsk.calc.keys {
        return Key::Pass;
    }

    // Mientras el motor no ha contestado las teclas SIGUEN siendo suyas aunque
    // no hagan nada: dejarlas caer a la linea escribiria un `7` dentro de un
    // comando que el dueno no esta mirando.
    if dsk.calc.waiting {
        return Key::Taken;
    }

    // El retroceso se atiende aqui y no en `pulsar` porque **no es un boton de
    // la cara**: no lo hay en el `.maqueta`. Es una afordancia del teclado, y
    // meterlo en la tabla de teclas seria inventarle una tecla al dibujo.
    if c == 0x08 || c == 0x7F {
        dsk.calc.backspace();
        paint_calc(p, &dsk.calc_pad, &dsk.calc, dsk.tick.calc_hover);
        return Key::Taken;
    }

    if let Some(t) = tecla_de_teclado(c) {
        pulsar(dsk, p, t);
    }
    // Suya aunque no signifique nada: si la calculadora tiene las teclas, las
    // tiene todas. Es la regla de la consola dicha para este panel.
    Key::Taken
}

/// Tecla del teclado -> **la misma tecla que devuelve el raton**.
///
/// Las dos entradas se juntan aqui, en un byte, y de ahi hacia abajo hay un
/// solo camino. `scene::calc::tecla_de` hace exactamente esto con el `id` del
/// `.maqueta`; esta es su gemela para el teclado.
fn tecla_de_teclado(c: u8) -> Option<u8> {
    Some(match c {
        b'0'..=b'9' => c,
        b'+' | b'-' | b'*' | b'/' | b'=' | b'%' | b'$' => c,
        // `n` de NEGAR. No hay tecla de `+/-` en ningun teclado, asi que se
        // elige una letra y se dice; la alternativa era dejar el signo solo
        // para el raton, que es lo que hace inutil media calculadora.
        b'n' | b'N' => b'~',
        // ** La COMA tambien es el punto decimal, y no es un capricho: en el
        // teclado espanol el separador del bloque numerico es la coma, y
        // `calcgui.cob` lee un `PIC S9(9)V99`, que quiere un PUNTO. Traducirlo
        // aqui es una linea; no traducirlo es que el bloque numerico no sirva.
        b'.' | b',' => b'.',
        // ENTRAR es el `=` de toda la vida.
        b'\n' | b'\r' => b'=',
        b'c' | b'C' => b'C',
        _ => return None,
    })
}

/// **Una tecla de la calculadora, venga de donde venga.**
///
/// Esto era el cuerpo del `if` del raton. Vive aqui desde que tiene DOS
/// llamadores: la copia en cada uno seria la misma cuenta escrita dos veces,
/// que es justo lo que MAQUETA acaba de borrar del pintado.
pub(crate) fn pulsar(dsk: &mut Desktop, p: &bmo::Pantalla, t: u8) {
    match t {
        b'C' => dsk.calc.clear(),
        b'+' => dsk.calc.operator(OP_SUMA),
        b'-' => dsk.calc.operator(OP_RESTA),
        b'*' => dsk.calc.operator(OP_POR),
        b'/' => dsk.calc.operator(OP_ENTRE),
        b'%' => dsk.calc.operator(OP_CIENTO),
        // El signo no sale de aqui: es edicion, y se ve al momento.
        b'~' => dsk.calc.negate(),
        // `$` no espera a un segundo operando ni a un `=`: lo que hay en el
        // visor ya es todo lo que necesita.
        b'$' => lanzar(dsk, p, OP_DINERO, true),
        b'=' => {
            if dsk.calc.op != 0 {
                let cod = dsk.calc.op;
                lanzar(dsk, p, cod, false);
            }
        }
        d => dsk.calc.feed(d),
    }
    paint_calc(p, &dsk.calc_pad, &dsk.calc, dsk.tick.calc_hover);
}

/// Lanzar el MOTOR y darle sus tres lineas por la consola.
///
/// ** Aqui es donde la cara deja de saber aritmetica y empieza a saber COBOL.
///
/// `unario` distingue las dos formas que tiene una pregunta:
///
/// ```text
///   binaria   izquierda  codigo  derecha    + - * / y %
///   unaria    el visor   codigo  0          $, que no calcula sino presenta
/// ```
///
/// El motor lee TRES lineas siempre, asi que la unaria manda un `0` de relleno
/// que su rama no mira. Mandar menos lineas segun el codigo obligaria a los dos
/// lados a ponerse de acuerdo sobre cuantas hay **antes** de saber cual es, y
/// eso es un protocolo que se desincroniza a la primera.
fn lanzar(dsk: &mut Desktop, p: &bmo::Pantalla, cod: u8, unario: bool) {
    // Sin los datos no hay pregunta que hacer. Lanzar el motor igual gastaria
    // un proceso entero para que conteste a medias.
    if unario {
        if dsk.calc.n == 0 {
            return;
        }
    } else if dsk.calc.saved_n == 0 || dsk.calc.n == 0 {
        return;
    }
    let cap = dsk.out.console.as_ref().map(|c| c.cap).unwrap_or(0);
    if bmo::ejecutar_en(b"cobol/calcgui.bex", cap).is_err() {
        paint_status(p, &dsk.run_box, "falta cobol/calcgui.bex", INK_BAD);
        return;
    }
    if let Some(cc) = dsk.out.console.as_ref() {
        if unario {
            cc.write(&dsk.calc.input[..dsk.calc.n]);
        } else {
            cc.write(&dsk.calc.saved_path[..dsk.calc.saved_n]);
        }
        cc.write(b"\n");
        cc.write(&[b'0' + cod]);
        cc.write(b"\n");
        if unario {
            cc.write(b"0");
        } else {
            cc.write(&dsk.calc.input[..dsk.calc.n]);
        }
        cc.write(b"\n");
    }
    dsk.calc.waiting = true;
    dsk.resp_n = 0;
}
