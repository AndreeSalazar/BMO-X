//! **Hay algo escrito que no hace nada?**
//!
//! Ninguna de estas tres rompe la imagen. Son errores igualmente, y por la regla
//! que ordena el proyecto entero: *nada que compile y no haga lo que dice*.
//!
//! Una linea aceptada que no se honra es la mentira que envejece sin avisar --
//! la misma clase que `INFO_ES_ESCRIBIBLE => 0`, un valor puesto por prudencia
//! que tres meses despues era falso y nadie se entero.

use bmo_maqueta_cascade::{Display, Position};
use bmo_maqueta_diag::Error;
use bmo_maqueta_layout::{Frame, Laid};

pub fn check(laid: &Laid, out: &mut Vec<Error>) {
    for f in laid.all() {
        gap_sin_flex(f, out);
        absoluta_sin_sitio(f, out);
        texto_sin_color(f, out);
    }
}

/// G. `gap` en una caja que no reparte nada.
fn gap_sin_flex(f: &Frame, out: &mut Vec<Error>) {
    if f.style.gap == 0 || f.style.display == Display::Flex {
        return;
    }
    out.push(Error::new(
        f.span,
        &format!("`gap:{}px` aqui no hace nada", f.style.gap),
        "`gap` solo separa elementos de un contenedor flex, y esta caja es `block`. \
         En un navegador tampoco haria nada -- la diferencia es que alli no te lo \
         dice nadie y aqui si.",
        "anadir `display:flex`, o quitar el `gap`.",
    ));
}

/// H. `position:absolute` sin decir donde.
fn absoluta_sin_sitio(f: &Frame, out: &mut Vec<Error>) {
    if f.style.position != Position::Absolute {
        return;
    }
    if f.style.left.is_some() && f.style.top.is_some() {
        return;
    }
    out.push(Error::new(
        f.span,
        "una caja absoluta tiene que decir `left` y `top`",
        "salirse del flujo es renunciar a que alguien te coloque. Sin las dos \
         coordenadas la caja cae en el 0 que puso el compilador, que no es una \
         decision de nadie.",
        "`left` y `top` en pixeles, contra el lienzo.",
    ));
}

/// F. Texto que nadie ha coloreado.
///
/// Consecuencia directa de no tener herencia: sin un color heredado del padre y
/// sin uno por defecto --que seria herencia de ninguna parte-- un texto sin
/// `color` no tiene ninguno. Es el precio del trato, y se cobra aqui en vez de
/// pintarse de un color que nadie eligio.
fn texto_sin_color(f: &Frame, out: &mut Vec<Error>) {
    let Some(t) = &f.text else { return };
    if t.is_empty() || f.style.color.is_some() {
        return;
    }
    out.push(Error::new(
        f.span,
        "este texto no tiene color",
        "MAQUETA no hereda, y no hay color por defecto: seria herencia de ninguna \
         parte -- un valor que nadie escribio, con pinta de intencional, que envejece \
         hacia la mentira. Sin `color` no hay con que pintar estas letras.",
        "una clase de la paleta -- `class=\"ink\"` o `class=\"ink-dim\"`, que estan en \
         `toolchain/tools/maqueta/tema/tema.maqueta` -- o un `color:#RRGGBB` propio.",
    ));
}
