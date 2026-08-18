//! **Los nombres responden?**
//!
//! Un `id` es la clave de la tabla de golpeo y un `nombre` de isla es por donde
//! otro proceso encuentra su rect. Los dos son promesas hacia fuera, y una
//! promesa repetida no falla: **contesta lo que no es**.

use bmo_maqueta_cascade::Cascaded;
use bmo_maqueta_diag::Error;
use bmo_maqueta_layout::Laid;

pub fn check(laid: &Laid, c: &Cascaded, out: &mut Vec<Error>) {
    unicos(laid, out);
    islas(laid, out);
    reglas(laid, c, out);
}

/// D. Dos cajas con el mismo `id`.
fn unicos(laid: &Laid, out: &mut Vec<Error>) {
    let mut vistos: Vec<&str> = Vec::new();
    for f in laid.all() {
        let Some(id) = f.id.as_deref() else { continue };
        if vistos.contains(&id) {
            out.push(Error::new(
                f.span,
                &format!("el id `{id}` ya estaba usado"),
                "el `id` es la clave de la tabla de golpeo. Con dos iguales, un clic \
                 contesta el primero que aparezca -- y cual sea eso depende del orden \
                 de pintado, que nadie escribio pensando en clics.",
                "un nombre distinto por caja. El `id` no estila, asi que no hay motivo \
                 para repetirlo.",
            ));
        } else {
            vistos.push(id);
        }
    }
}

/// E. Una isla sin sitio o con el nombre repetido.
fn islas(laid: &Laid, out: &mut Vec<Error>) {
    let mut vistos: Vec<&str> = Vec::new();
    for (nombre, rect) in laid.islands() {
        if vistos.contains(&nombre) {
            let f = laid
                .all()
                .into_iter()
                .find(|f| f.island.as_deref() == Some(nombre))
                .expect("la isla salio de este arbol");
            out.push(Error::new(
                f.span,
                &format!("la isla `{nombre}` ya estaba"),
                "el nombre es por donde el proceso de fuera encuentra su rect. Con dos \
                 iguales, el que rellene una rellenaria la otra.",
                "un nombre por isla.",
            ));
            continue;
        }
        vistos.push(nombre);
        if rect.w == 0 || rect.h == 0 {
            let f = laid
                .all()
                .into_iter()
                .find(|f| f.island.as_deref() == Some(nombre))
                .expect("la isla salio de este arbol");
            out.push(Error::new(
                f.span,
                &format!("la isla `{nombre}` mide {}x{}", rect.w, rect.h),
                "una isla no se maqueta segun lo que le metan dentro: su tamano lo pone \
                 LA MAQUETA. Al reves, una app colgada dejaria el escritorio sin \
                 calcular -- que es la decision 2 de `PLAN_DIRECTOR.md`.",
                "darle `width` y `height`, o dejar que su contenedor flex la estire.",
            ));
        }
    }
}

/// I y J: lo que el hijo apunto al casar reglas con cajas.
///
/// Los ficheros sin cajas ya quedaron fuera en `judge` -- ver `es_fragmento`.
fn reglas(_laid: &Laid, c: &Cascaded, out: &mut Vec<Error>) {
    for f in &c.dead_rules {
        out.push(Error::new(
            f.span,
            &format!("la regla `{}` no llega a ninguna caja", f.what),
            "una regla que no casa con nada dice algo y no hace nada, que es \
             exactamente lo que este compilador existe para que no ocurra. Y casi \
             siempre es una errata en el nombre.",
            "corregir el nombre, ponerselo a alguna caja, o borrar la regla.",
        ));
    }
    for f in &c.orphan_classes {
        out.push(Error::new(
            f.span,
            &format!("la clase `{}` no la define ninguna regla", f.what),
            "la caja la lleva puesta y no le hace nada. Suele ser el otro lado de la \
             misma errata.",
            "escribir la regla, o quitar la clase.",
        ));
    }
}
