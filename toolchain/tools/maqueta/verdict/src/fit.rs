//! **Cabe todo?**
//!
//! Las tres comprobaciones que miran los rects ya calculados y no repiten ni una
//! resta. La segunda es la que mas veces va a saltar y la que mas vale.

use bmo_maqueta_cascade::Position;
use bmo_maqueta_diag::Error;
use bmo_maqueta_layout::{Frame, Laid, Rect, GLIFO_ALTO, GLIFO_ANCHO};

pub fn check(laid: &Laid, out: &mut Vec<Error>) {
    let canvas = Rect {
        x: 0,
        y: 0,
        w: laid.canvas.0,
        h: laid.canvas.1,
    };
    walk(&laid.root, &canvas, out);
}

fn walk(f: &Frame, canvas: &Rect, out: &mut Vec<Error>) {
    for c in &f.children {
        let (limite, quien) = if c.style.position == Position::Absolute {
            (canvas, "el lienzo")
        } else {
            (&f.content, "su padre")
        };
        if !c.rect.inside(limite) {
            out.push(fuera(c, limite, quien));
        }
        walk(c, canvas, out);
    }
    cabe_el_texto(f, out);
    no_esta_vacia(f, out);
}

/// A. Una caja fuera de su sitio.
fn fuera(c: &Frame, limite: &Rect, quien: &str) -> Error {
    Error::new(
        c.span,
        &format!(
            "esta caja se sale de {quien} -- esta en ({}, {}) y mide {}x{}, y el sitio \
             va de ({}, {}) a ({}, {})",
            c.rect.x,
            c.rect.y,
            c.rect.w,
            c.rect.h,
            limite.x,
            limite.y,
            limite.right(),
            limite.bottom()
        ),
        "BMO-X no recorta ni desplaza: lo que se sale se pinta encima de lo de al \
         lado, o fuera de la pantalla. No hay `overflow` que lo tape, y por eso esto \
         es un error y no un caso a manejar en ejecucion.",
        "dar sitio al padre (mas `width`/`height`, o menos `padding`), o encoger la \
         caja. Si lo que se queria era salirse a proposito, `position:absolute`.",
    )
}

/// ★ B. El texto que no cabe.
///
/// Es la unica clase de fallo de este sistema que **se ve bonita en pantalla y
/// esta mal**: un navegador lo esconde reajustando lineas, y BMO-X no puede --
/// la fuente no parte palabras, asi que las letras que sobran se pintan encima
/// del borde y nadie avisa.
fn cabe_el_texto(f: &Frame, out: &mut Vec<Error>) {
    let Some(t) = &f.text else { return };
    if t.is_empty() {
        return;
    }
    let ancho = t.len() as u32 * GLIFO_ANCHO;
    if ancho > f.content.w {
        out.push(Error::new(
            f.span,
            &format!(
                "el texto no cabe -- mide {ancho} px ({} letras x {GLIFO_ANCHO}) y la \
                 caja da {} px",
                t.len(),
                f.content.w
            ),
            "la fuente de BMO-X no parte palabras ni reajusta lineas, asi que las \
             letras que sobran se pintan por encima del borde. Un navegador lo \
             esconde y aqui se ve, y por eso es la comprobacion que mas vale: es el \
             unico fallo que queda BONITO en pantalla estando mal.",
            &format!(
                "un `width` de {ancho} px o mas, menos `padding`, o menos texto.",
            ),
        ));
    }
    if GLIFO_ALTO > f.content.h {
        out.push(Error::new(
            f.span,
            &format!(
                "el texto no cabe de alto -- una linea son {GLIFO_ALTO} px y la caja da {}",
                f.content.h
            ),
            "la fuente mide lo que mide y no se escala: hay una sola, de mapa de bits.",
            &format!("un `height` de {GLIFO_ALTO} px o mas."),
        ));
    }
}

/// C. Una caja de tamano cero.
fn no_esta_vacia(f: &Frame, out: &mut Vec<Error>) {
    if f.rect.w != 0 && f.rect.h != 0 {
        return;
    }
    out.push(Error::new(
        f.span,
        &format!("esta caja mide {}x{} y no se va a ver", f.rect.w, f.rect.h),
        "casi siempre es una propiedad que se olvido, no una intencion. Y como no \
         pinta nada ni ocupa sitio, no hay forma de darse cuenta mirando la pantalla.",
        "declarar `width` y `height`, o meterle contenido que le de tamano. Si de \
         verdad no tenia que verse, borrarla.",
    ))
}
