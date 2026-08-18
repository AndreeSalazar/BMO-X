//! **COMO se escribe en Rust.** Traduce; no decide.
//!
//! Lo que hay que dibujar lo dice `orden.rs`, y lo que cae dentro de un rect lo
//! dice `recorte.rs`. Aqui solo se convierte una lista en texto. Ese corte es lo
//! que permite que el recurso BEF y el reflejo en PPM sean **otro traductor** y
//! no otra deduccion.
//!
//! ## Por que codigo y no datos
//!
//! Una tabla habria pedido tipos --`Caja`, `Letras`, `Golpe`-- que tendrian que
//! existir **en los dos lados**: aqui para escribirlos y en Ring 3 para leerlos.
//! Eso es una segunda copia de un contrato, el fallo que este arbol ya pago con
//! `bmo.h`. Emitir llamadas no tiene contrato: usa `Pantalla`, que ya existe.
//!
//! ## [!] Lo que SALE tiene que ser ASCII puro
//!
//! Este fichero puede usar los simbolos de la casa; lo que emite **no**, porque
//! se convierte en un fuente de BMO-X. Se colo un simbolo una vez y lo cazo una
//! prueba, no una lectura.

use bmo_maqueta_layout::{Laid, Rect};
use std::fmt::Write;

use crate::orden::{lista, Estado, Orden, Trazo};

/// Genera un modulo de Rust a partir de una maquetacion resuelta.
///
/// `origen` es la ruta del `.maqueta`, **relativa a la raiz del repositorio**.
/// Que sea relativa no es estetica: un artefacto cuya primera linea depende de
/// como se tecleo el comando **no se puede comparar**, y comparar es lo unico
/// que impide que la cara pintada y su `.maqueta` se separen.
pub fn modulo(origen: &str, l: &Laid) -> String {
    let ordenes = lista(l);
    let mut s = String::new();
    cabecera(&mut s, origen, l);
    pintar(&mut s, &ordenes);
    pintar_en(&mut s, &ordenes);
    realce(&mut s, &ordenes);
    golpe(&mut s, l);
    islas(&mut s, l);
    s
}

fn cabecera(s: &mut String, origen: &str, l: &Laid) {
    let _ = write!(
        s,
        "//! GENERADO POR MAQUETA DESDE `{origen}` -- NO EDITAR A MANO.\n\
         //!\n\
         //! Lo que se edita es el `.maqueta`. Cambiar esto es escribir una verdad\n\
         //! que la siguiente compilacion borra.\n\
         //!\n\
         //! Todas las coordenadas son relativas al origen que se pase, asi que\n\
         //! este modulo no sabe donde esta la ventana -- igual que no lo sabia el\n\
         //! `.maqueta`.\n\
         \n\
         #![allow(clippy::identity_op, clippy::erasing_op)]\n\
         // [!] `dead_code` aparte, y con motivo: este modulo ofrece la superficie\n\
         // ENTERA de la maquetacion --pintar, recortar, realzar, golpear, las\n\
         // islas-- y cual de esas usa la app es cosa de la app. Recortar lo que\n\
         // hoy no se llama obligaria a regenerar el dia que alguien lo use.\n\
         #![allow(dead_code)]\n\
         \n\
         use bmo_userland as bmo;\n\
         \n\
         /// El tamano que MAQUETA dedujo del arbol. Nadie lo escribio.\n\
         pub const ANCHO: u32 = {};\n\
         pub const ALTO: u32 = {};\n\
         \n",
        l.canvas.0, l.canvas.1
    );
}

// ------------------------------------------------------------------------
//  Un trazo, escrito
// ------------------------------------------------------------------------

/// La llamada que pinta este trazo, sin recortar.
fn llamada(t: &Trazo) -> String {
    match t {
        Trazo::Rect { r, color } => format!(
            "p.rect(ox + {}, oy + {}, {}, {}, 0x{color:08X});",
            r.x, r.y, r.w, r.h
        ),
        Trazo::Texto { r, texto, color } => format!(
            "p.texto(ox + {}, oy + {}, {texto:?}, 0x{color:08X});",
            r.x, r.y
        ),
    }
}

fn area(t: &Trazo) -> Rect {
    t.area()
}

// ------------------------------------------------------------------------
//  Los cuatro pintados
// ------------------------------------------------------------------------

fn pintar(s: &mut String, ordenes: &[Orden]) {
    s.push_str(
        "/// Pinta la maquetacion entera con su esquina superior izquierda en\n\
         /// `(ox, oy)`. El orden es el del fichero, que ES el orden de pintado.\n\
         pub fn pintar(p: &bmo::Pantalla, ox: u32, oy: u32) {\n",
    );
    let mut ultimo = String::new();
    for o in ordenes.iter().filter(|o| o.estado == Estado::Reposo) {
        if o.de != ultimo {
            let _ = writeln!(s, "    // {}", o.de);
            ultimo = o.de.clone();
        }
        let _ = writeln!(s, "    {}", llamada(&o.trazo));
    }
    s.push_str("}\n\n");
}

/// ** El pintado RECORTADO, que es lo que hace barato reparar un danio.
fn pintar_en(s: &mut String, ordenes: &[Orden]) {
    s.push_str(
        "/// Repinta SOLO lo que cae dentro de `(cx, cy, cw, ch)`, en coordenadas\n\
         /// de pantalla. Para devolver el fondo de un area sin repintarlo todo.\n\
         ///\n\
         /// ** Por que existe, con el numero: devolver fondo preguntando el color\n\
         /// PIXEL A PIXEL cuesta ~325.000 escrituras por borrado, que a los\n\
         /// ~300 MB/s medidos en el Ryzen son 4,33 ms -- la cuarta parte de un\n\
         /// fotograma de 60 Hz, y arrastrar hace uno por evento de raton. Esto son\n\
         /// unas pocas llamadas a `rect`, que escriben por filas.\n\
         ///\n\
         /// Los rectangulos se RECORTAN; el texto entra entero o no entra, porque\n\
         /// medio glifo no se puede pintar.\n\
         pub fn pintar_en(p: &bmo::Pantalla, ox: u32, oy: u32, cx: u32, cy: u32, cw: u32, ch: u32) {\n",
    );
    let reposo: Vec<&Orden> = ordenes.iter().filter(|o| o.estado == Estado::Reposo).collect();
    if reposo.is_empty() {
        s.push_str("    let _ = (p, ox, oy, cx, cy, cw, ch);\n");
    }
    let mut ultimo = String::new();
    for o in reposo {
        if o.de != ultimo {
            let _ = writeln!(s, "    // {}", o.de);
            ultimo = o.de.clone();
        }
        let r = area(&o.trazo);
        match &o.trazo {
            Trazo::Rect { color, .. } => {
                let _ = writeln!(
                    s,
                    "    if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + {}, oy + {}, {}, {}) {{\n\
                     \x20       p.rect(x, y, w, h, 0x{color:08X});\n\
                     \x20   }}",
                    r.x, r.y, r.w, r.h
                );
            }
            Trazo::Texto { texto, color, .. } => {
                let _ = writeln!(
                    s,
                    "    if cruza(cx, cy, cw, ch, ox + {}, oy + {}, {}, {}) {{\n\
                     \x20       p.texto(ox + {}, oy + {}, {texto:?}, 0x{color:08X});\n\
                     \x20   }}",
                    r.x, r.y, r.w, r.h, r.x, r.y
                );
            }
        }
    }
    s.push_str("}\n\n");

    s.push_str(
        "/// La parte de un rectangulo que cae dentro del limite. `None` si no se\n\
         /// tocan -- y tocarse por el borde NO es tocarse: `[x0, x1)`, medio\n\
         /// abierto, la misma regla que el recorte de `bmo-dibujo`. Si el borde\n\
         /// contara, cada reparacion repintaria una fila de mas y se veria como\n\
         /// una costura.\n\
         fn corte(cx: u32, cy: u32, cw: u32, ch: u32, x: u32, y: u32, w: u32, h: u32)\n\
         \x20   -> Option<(u32, u32, u32, u32)>\n\
         {\n\
         \x20   let x0 = if x > cx { x } else { cx };\n\
         \x20   let y0 = if y > cy { y } else { cy };\n\
         \x20   let x1 = if x + w < cx + cw { x + w } else { cx + cw };\n\
         \x20   let y1 = if y + h < cy + ch { y + h } else { cy + ch };\n\
         \x20   if x1 > x0 && y1 > y0 {\n\
         \x20       Some((x0, y0, x1 - x0, y1 - y0))\n\
         \x20   } else {\n\
         \x20       None\n\
         \x20   }\n\
         }\n\
         \n\
         /// Se tocan? Para el texto, que es atomico.\n\
         fn cruza(cx: u32, cy: u32, cw: u32, ch: u32, x: u32, y: u32, w: u32, h: u32) -> bool {\n\
         \x20   corte(cx, cy, cw, ch, x, y, w, h).is_some()\n\
         }\n\n",
    );
}

/// El estado "encima", que es todo lo que MAQUETA sabe de animacion.
///
/// ** No recoloca nada, porque no puede: el padre no deja que una regla `:hover`
/// toque mas que pintura. Por eso cuesta un rect y no un recalculo.
fn realce(s: &mut String, ordenes: &[Orden]) {
    s.push_str(
        "/// Repinta la caja `id` con sus colores de `:hover`. Llamalo cuando el\n\
         /// puntero entre, y `pintar` cuando salga.\n\
         pub fn realce(p: &bmo::Pantalla, ox: u32, oy: u32, id: &str) {\n",
    );
    let encima: Vec<&Orden> = ordenes.iter().filter(|o| o.estado == Estado::Encima).collect();
    if encima.is_empty() {
        s.push_str("    let _ = (p, ox, oy, id);\n");
    }
    let mut abierto = String::new();
    for o in encima {
        if o.de != abierto {
            if !abierto.is_empty() {
                s.push_str("        return;\n    }\n");
            }
            // `de` es `#k_c`; el `id` del golpeo es `k_c`.
            let _ = writeln!(s, "    if id == {:?} {{", o.de.trim_start_matches('#'));
            abierto = o.de.clone();
        }
        let _ = writeln!(s, "        {}", llamada(&o.trazo));
    }
    if !abierto.is_empty() {
        s.push_str("        return;\n    }\n");
    }
    s.push_str("}\n\n");
}

// ------------------------------------------------------------------------
//  Golpeo e islas
// ------------------------------------------------------------------------

fn golpe(s: &mut String, l: &Laid) {
    let hits = l.hits();
    s.push_str(
        "/// Que `id` hay bajo `(px, py)`, con la maquetacion puesta en `(ox, oy)`.\n\
         ///\n\
         /// ** Sale de la MISMA pasada que `pintar`, asi que no hay una segunda\n\
         /// aritmetica que pueda discrepar: el boton que se dibuja aqui responde\n\
         /// aqui, por construccion y no por cuidado.\n\
         pub fn golpe(ox: u32, oy: u32, px: u32, py: u32) -> Option<&'static str> {\n",
    );
    if hits.is_empty() {
        s.push_str("    let _ = (ox, oy, px, py);\n");
    }
    for (id, r) in &hits {
        let _ = writeln!(
            s,
            "    if px >= ox + {} && px < ox + {} && py >= oy + {} && py < oy + {} {{\n\
             \x20       return Some({id:?});\n\
             \x20   }}",
            r.x,
            r.right(),
            r.y,
            r.bottom()
        );
    }
    s.push_str("    None\n}\n\n");

    s.push_str(
        "/// Esta `(px, py)` dentro de la maquetacion?\n\
         pub fn dentro(ox: u32, oy: u32, px: u32, py: u32) -> bool {\n\
         \x20   px >= ox && px < ox + ANCHO && py >= oy && py < oy + ALTO\n\
         }\n\n",
    );
}

fn islas(s: &mut String, l: &Laid) {
    let islas = l.islands();
    let _ = writeln!(
        s,
        "/// Los huecos que rellena otro proceso: nombre, x, y, ancho, alto.\n\
         ///\n\
         /// Relativos al origen. Una isla es una superficie de `PLAN_DIRECTOR.md`\n\
         /// vista desde la maqueta: aqui solo se dice DONDE va.\n\
         pub const ISLAS: [(&str, u32, u32, u32, u32); {}] = [",
        islas.len()
    );
    for (nombre, r) in &islas {
        let _ = writeln!(s, "    ({nombre:?}, {}, {}, {}, {}),", r.x, r.y, r.w, r.h);
    }
    s.push_str("];\n\n");

    s.push_str(
        "/// El rect de una isla por su nombre: x, y, ancho, alto.\n\
         pub fn isla(nombre: &str) -> Option<(u32, u32, u32, u32)> {\n\
         \x20   let mut k = 0;\n\
         \x20   while k < ISLAS.len() {\n\
         \x20       let (n, x, y, w, h) = ISLAS[k];\n\
         \x20       if n == nombre {\n\
         \x20           return Some((x, y, w, h));\n\
         \x20       }\n\
         \x20       k += 1;\n\
         \x20   }\n\
         \x20   None\n\
         }\n\n",
    );

    s.push_str(
        "/// Repinta el fondo de una isla, para borrar lo que hubiera dentro.\n\
         ///\n\
         /// ** Existe para que quien rellena la isla NO tenga que saber su color.\n\
         /// Copiarlo en Rust seria una segunda verdad, y el dia que cambie el\n\
         /// `.maqueta` una de las dos se quedaria vieja sin avisar.\n\
         pub fn limpiar_isla(p: &bmo::Pantalla, ox: u32, oy: u32, nombre: &str) {\n",
    );
    let con_fondo: Vec<_> = l
        .all()
        .into_iter()
        .filter(|f| f.island.is_some() && f.style.background.is_some())
        .collect();
    if con_fondo.is_empty() {
        s.push_str("    let _ = (p, ox, oy, nombre);\n");
    }
    for f in con_fondo {
        let n = f.island.as_deref().expect("filtrado arriba");
        let fondo = f.style.background.expect("filtrado arriba");
        let d = f.style.border_width;
        let _ = writeln!(
            s,
            "    if nombre == {n:?} {{\n\
             \x20       p.rect(ox + {}, oy + {}, {}, {}, 0x{fondo:08X});\n\
             \x20       return;\n\
             \x20   }}",
            f.rect.x + d as i32,
            f.rect.y + d as i32,
            f.rect.w.saturating_sub(d * 2),
            f.rect.h.saturating_sub(d * 2),
        );
    }
    s.push_str("}\n");
}
