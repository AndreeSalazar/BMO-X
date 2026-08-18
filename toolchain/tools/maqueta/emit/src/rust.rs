//! **Emit the Rust that a person writes today, already unrolled.**
//!
//! ## Por que codigo y no datos
//!
//! Una tabla de datos habria pedido tipos --`Caja`, `Letras`, `Golpe`-- y esos
//! tipos tendrian que existir **en los dos lados**: aqui, para escribirlos, y en
//! Ring 3, para leerlos. Eso es **una segunda copia de un contrato**, que es el
//! fallo que este arbol ya ha pagado dos veces (`bmo.h` acabo siendo la cuarta,
//! y `GLIFO_ANCHO` lo es hoy con un guardian encima).
//!
//! Emitir llamadas no tiene contrato: usa `Pantalla`, que ya existe, y lo que
//! sale se puede **poner al lado de `paint_calc` y comparar linea a linea**.
//!
//! ⚠️ Y lo que esto NO da: no se puede cambiar sin recompilar. La version que si
//! --el recurso BEF 0x0B, escalon 8-- es un emisor mas, **y no toca ninguna de
//! las cinco generaciones**, porque ninguna sabe que existe un emisor.
//!
//! ## Lo que reemplaza, con nombre
//!
//! ```text
//!    paint_calc()          -> `pintar()`, ya desenrollado
//!    button() + key_at()   -> `golpe()`, de la MISMA pasada
//!    contains()            -> `dentro()`
//!    CALC_BTN, CALC_GAP,
//!    CALC_COLS/ROWS, CALC_KEYS, CalcPad  -> nada. Ya no hacen falta.
//! ```

//! ## [!] Lo que SALE tiene que ser ASCII puro
//!
//! Este fichero puede usar los simbolos de la casa (★, ⚠️) porque es fuente del
//! anfitrion. Lo que emite **no**: se convierte en un fuente de BMO-X, y ahi la
//! regla del 2026-08-08 dice ASCII. Una sola letra acentuada en un literal llego
//! a hacer crecer un `.bex` de 512 bytes a 492.032.
//!
//! Se me colo un `★` en un comentario generado y lo cazo la prueba
//! `lo_generado_esta_equilibrado_y_no_tiene_sorpresas`, no una lectura.

use bmo_maqueta_layout::{Frame, Laid};
use std::fmt::Write;

/// Genera un modulo de Rust a partir de una maquetacion resuelta.
///
/// `origen` es la ruta del `.maqueta`, y va dentro del fichero generado: un
/// fichero que no dice de donde salio invita a editarlo.
pub fn modulo(origen: &str, l: &Laid) -> String {
    let mut s = String::new();
    cabecera(&mut s, origen, l);
    pintar(&mut s, l);
    realce(&mut s, l);
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
         // ENTERA de la maquetacion --pintar, realzar, golpear, las islas-- y\n\
         // cual de esas usa la app es cosa de la app. Recortar lo que hoy no se\n\
         // llama obligaria a regenerar el dia que alguien empiece a usarlo.\n\
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

fn pintar(s: &mut String, l: &Laid) {
    s.push_str(
        "/// Pinta la maquetacion entera con su esquina superior izquierda en\n\
         /// `(ox, oy)`. El orden es el del fichero, que ES el orden de pintado.\n\
         pub fn pintar(p: &bmo::Pantalla, ox: u32, oy: u32) {\n",
    );
    for f in l.all() {
        let etiqueta = nombre_de(f);
        let mut escrito = false;

        // El borde primero y el fondo encima, que es como lo hace `calc.rs`:
        // dos rects concentricos en vez de cuatro tiras.
        if let Some(borde) = f.style.border_color {
            if f.style.border_width > 0 {
                let _ = writeln!(s, "    // {etiqueta}");
                escrito = true;
                let _ = writeln!(
                    s,
                    "    p.rect(ox + {}, oy + {}, {}, {}, 0x{:08X});",
                    f.rect.x, f.rect.y, f.rect.w, f.rect.h, borde
                );
            }
        }
        if let Some(fondo) = f.style.background {
            if !escrito {
                let _ = writeln!(s, "    // {etiqueta}");
                escrito = true;
            }
            let d = f.style.border_width;
            let _ = writeln!(
                s,
                "    p.rect(ox + {}, oy + {}, {}, {}, 0x{:08X});",
                f.rect.x + d as i32,
                f.rect.y + d as i32,
                f.rect.w.saturating_sub(d * 2),
                f.rect.h.saturating_sub(d * 2),
                fondo
            );
        }
        if let (Some(t), Some(at), Some(color)) = (&f.text, f.text_at, f.style.color) {
            if !escrito {
                let _ = writeln!(s, "    // {etiqueta}");
            }
            let _ = writeln!(
                s,
                "    p.texto(ox + {}, oy + {}, {:?}, 0x{:08X});",
                at.x, at.y, t, color
            );
        }
    }
    s.push_str("}\n\n");
}

/// El estado "encima", que es todo lo que MAQUETA sabe de animacion.
///
/// ** Repinta UNA caja con sus colores de `:hover`. No recoloca nada, porque no
/// puede: el padre no deja que una regla `:hover` toque mas que pintura. Por eso
/// esto cuesta un rect y no un recalculo.
///
/// Lo que se mueve de verdad --una transicion, un desplazamiento-- es Rust en el
/// bucle de fotograma, y esa frontera es lo que impide que esto acabe siendo un
/// navegador.
fn realce(s: &mut String, l: &Laid) {
    let con_hover: Vec<_> = l
        .all()
        .into_iter()
        .filter(|f| f.hover.is_some() && f.id.is_some())
        .collect();

    s.push_str(
        "/// Repinta la caja `id` con sus colores de `:hover`. Llamalo cuando el\n\
         /// puntero entre, y `pintar` cuando salga.\n\
         pub fn realce(p: &bmo::Pantalla, ox: u32, oy: u32, id: &str) {\n",
    );
    if con_hover.is_empty() {
        s.push_str("    let _ = (p, ox, oy, id);\n");
    }
    for f in &con_hover {
        let h = f.hover.expect("filtrado arriba");
        let id = f.id.as_deref().expect("filtrado arriba");
        let _ = writeln!(s, "    if id == {id:?} {{");
        if let (Some(borde), true) = (h.border_color, h.border_width > 0) {
            let _ = writeln!(
                s,
                "        p.rect(ox + {}, oy + {}, {}, {}, 0x{:08X});",
                f.rect.x, f.rect.y, f.rect.w, f.rect.h, borde
            );
        }
        if let Some(fondo) = h.background {
            let d = h.border_width;
            let _ = writeln!(
                s,
                "        p.rect(ox + {}, oy + {}, {}, {}, 0x{:08X});",
                f.rect.x + d as i32,
                f.rect.y + d as i32,
                f.rect.w.saturating_sub(d * 2),
                f.rect.h.saturating_sub(d * 2),
                fondo
            );
        }
        if let (Some(t), Some(at), Some(color)) = (&f.text, f.text_at, h.color) {
            let _ = writeln!(
                s,
                "        p.texto(ox + {}, oy + {}, {:?}, 0x{:08X});",
                at.x, at.y, t, color
            );
        }
        s.push_str("        return;\n    }\n");
    }
    s.push_str("}\n\n");
}

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
             \x20       return Some({:?});\n\
             \x20   }}",
            r.x,
            r.right(),
            r.y,
            r.bottom(),
            id
        );
    }
    s.push_str("    None\n}\n\n");

    let _ = write!(
        s,
        "/// Esta `(px, py)` dentro de la maquetacion?\n\
         pub fn dentro(ox: u32, oy: u32, px: u32, py: u32) -> bool {{\n\
         \x20   px >= ox && px < ox + ANCHO && py >= oy && py < oy + ALTO\n\
         }}\n\n"
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
        let _ = writeln!(
            s,
            "    ({:?}, {}, {}, {}, {}),",
            nombre, r.x, r.y, r.w, r.h
        );
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
             \x20       p.rect(ox + {}, oy + {}, {}, {}, 0x{:08X});\n\
             \x20       return;\n\
             \x20   }}",
            f.rect.x + d as i32,
            f.rect.y + d as i32,
            f.rect.w.saturating_sub(d * 2),
            f.rect.h.saturating_sub(d * 2),
            fondo
        );
    }
    s.push_str("}\n");
}

/// Como se llama una caja en un comentario: su `id`, su isla, o su etiqueta.
fn nombre_de(f: &Frame) -> String {
    if let Some(id) = &f.id {
        return format!("#{id}");
    }
    if let Some(n) = &f.island {
        return format!("isla {n}");
    }
    f.tag.name().to_string()
}
