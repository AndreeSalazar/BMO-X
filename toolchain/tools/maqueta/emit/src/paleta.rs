//! **La paleta: de `tema.maqueta` a constantes de Rust.**
//!
//! # La deuda que esto cierra, y estaba escrita
//!
//! La cabecera de `tema/tema.maqueta` lo confesaba desde el 2026-08-17:
//!
//! > *"Mientras no exista el emisor, este fichero es la FUENTE y las constantes
//! > de Rust son la copia. Dos sitios con la misma verdad es la deuda conocida
//! > de esta fase, y se cierra en el escalon 6."*
//!
//! El escalon 6 existe --`calc_gen.rs` son 500 lineas generadas-- asi que esa
//! frase estaba caducada. Aqui se cobra.
//!
//! # Por que esto NO es el emisor de siempre
//!
//! `rust::modulo` emite **maquetacion**: donde cae cada caja. Y `tema.maqueta`
//! **no tiene una sola caja** -- es un `<style>` y nada mas. Pasarlo por el
//! emisor de siempre daria un modulo que pinta el vacio.
//!
//! ** Son dos preguntas distintas sobre el mismo fichero: *"donde va esto"* y
//! *"de que color es"*. Y la segunda es la que hace falta primero, porque el
//! color lo comparten las diecisiete caras del escritorio y la maquetacion no.
//!
//! # ** EL NOMBRE SE DEDUCE, NO SE ELIGE
//!
//! ```text
//!    .ink-dim   { color:#8A9BB4 }              ->  INK_DIM
//!    .box       { background-color:#1E2534 }   ->  BOX_FONDO
//!               { border-color:#333D52 }       ->  BOX_BORDE
//! ```
//!
//! La clase en mayusculas con guiones a barras bajas, y **el sufijo solo
//! aparece cuando hace falta**: `color` es el caso normal y no lo lleva; el
//! fondo y el borde si, porque una clase puede traer los tres.
//!
//! *** Que se deduzca es lo que impide una tabla de traduccion en el emisor --
//! y una tabla asi seria un TERCER sitio con la misma verdad, que es el
//! problema que este fichero viene a quitar, no a mover.
//!
//! # Lo que NO emite, dicho para que nadie lo busque
//!
//! Anchos, radios y todo lo que no sea un color. No por dificultad: un
//! `border-width:1px` sin la caja a la que pertenece no significa nada, y las
//! cajas las emite `rust::modulo`. Un numero suelto con nombre bonito es la
//! clase de constante que despues nadie sabe si puede cambiar.

use bmo_maqueta_node::{Document, Prop, Selector, Value};

/// De `ink-dim` a `INK_DIM`.
fn en_mayusculas(clase: &str) -> String {
    clase
        .chars()
        .map(|c| if c == '-' { '_' } else { c.to_ascii_uppercase() })
        .collect()
}

/// El sufijo de cada propiedad de color, o `None` si no es un color.
///
/// `Color` no lleva sufijo: es el caso normal --la tinta-- y `INK_DIM_COLOR`
/// solo seria mas largo.
fn sufijo(p: Prop) -> Option<&'static str> {
    match p {
        Prop::Color => Some(""),
        Prop::BackgroundColor => Some("_FONDO"),
        Prop::BorderColor => Some("_BORDE"),
        _ => None,
    }
}

/// **Las constantes de color de un documento, en Rust.**
///
/// `origen` es de donde salio, y va en la primera linea: un artefacto generado
/// que no dice quien lo genero es un artefacto que alguien va a editar a mano.
pub fn paleta(origen: &str, doc: &Document) -> String {
    let mut s = String::new();
    s.push_str(&alloc_cabecera(origen));

    let mut cuantas = 0usize;
    for regla in &doc.rules {
        // ** Las reglas de `:hover` se saltan y se dice por que: un color que
        // solo existe mientras el raton esta encima no es parte de la paleta,
        // es parte de la INTERACCION -- y esa vive en la cara, no aqui.
        if regla.hover {
            continue;
        }
        for sel in &regla.selectors {
            let Selector::Class(clase) = sel else { continue };
            for d in &regla.decls {
                let (Some(suf), Value::Color(c)) = (sufijo(d.prop), d.value) else {
                    continue;
                };
                s.push_str("pub const ");
                s.push_str(&en_mayusculas(clase));
                s.push_str(suf);
                s.push_str(": u32 = 0x00");
                s.push_str(&hex6(c));
                s.push_str(";\n");
                cuantas += 1;
            }
        }
    }

    if cuantas == 0 {
        // ** Un fichero de tema sin un solo color es casi seguro un error de
        // quien lo escribio, no una paleta vacia a proposito. Se dice DENTRO
        // del artefacto, donde lo va a leer quien se pregunte por que no
        // compila lo que esperaba.
        s.push_str("\n// [!] Este `.maqueta` no declara NINGUN color de clase.\n");
    }
    s
}

fn hex6(c: u32) -> String {
    const D: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(7);
    for i in (0..6).rev() {
        let n = ((c >> (i * 4)) & 0xF) as usize;
        out.push(D[n] as char);
        // ** UN SOLO guion bajo, y detras del segundo digito. El literal que
        // quiere Rust es `0x008A_9BB4`: el `00` del alfa va pegado a los dos
        // digitos del rojo, y el resto entero detras. Con dos guiones
        // --`0x008A_9B_B4`-- tambien compila y vale lo mismo, y por eso el
        // primer intento salio mal sin que nada fallara: la unica prueba es
        // comparar el TEXTO contra el que ya usa el escritorio.
        if i == 4 {
            out.push('_');
        }
    }
    out
}

fn alloc_cabecera(origen: &str) -> String {
    let mut s = String::new();
    s.push_str("//! GENERADO POR MAQUETA DESDE `");
    s.push_str(origen);
    s.push_str("` -- NO EDITAR A MANO.\n");
    s.push_str("//!\n");
    s.push_str("//! Lo que se edita es el `.maqueta`. Cambiar esto es escribir una verdad\n");
    s.push_str("//! que la siguiente compilacion borra.\n");
    s.push_str("//!\n");
    s.push_str("//! ** LA PALETA DE BMO-X EN UN SITIO. Antes eran 62 constantes de color\n");
    s.push_str("//! repartidas por quince ficheros, 33 de ellas usadas UNA vez -- y cada\n");
    s.push_str("//! panel nuevo se inventaba las suyas porque no habia donde consultarlas.\n");
    s.push_str("//!\n");
    s.push_str("//! El formato es `0x00RRGGBB`, que es el que quiere el framebuffer.\n\n");
    s.push_str("#![allow(dead_code)]\n\n");
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn de(src: &str) -> String {
        let doc = bmo_maqueta_node::parse(src.as_bytes()).expect("el tema no compila");
        paleta("prueba.maqueta", &doc)
    }

    #[test]
    fn el_nombre_sale_de_la_clase() {
        let r = de("<maqueta><style>.ink-dim { color:#8A9BB4 }</style></maqueta>");
        assert!(r.contains("pub const INK_DIM: u32 = 0x008A_9BB4;"), "{r}");
    }

    /// *** Los tres colores de una misma clase salen con nombres DISTINTOS.
    ///
    /// Sin el sufijo, `.box` con fondo y borde produciria la misma constante
    /// dos veces y la segunda taparia a la primera **sin que nada fallara**:
    /// el fichero compilaria y el borde tendria el color del fondo.
    #[test]
    fn una_clase_con_fondo_y_borde_no_se_pisa() {
        let r = de(
            "<maqueta><style>.box { background-color:#1E2534; border-color:#333D52 }</style></maqueta>",
        );
        assert!(r.contains("pub const BOX_FONDO: u32 = 0x001E_2534;"), "{r}");
        assert!(r.contains("pub const BOX_BORDE: u32 = 0x0033_3D52;"), "{r}");
    }

    /// Lo que no es color no entra, aunque este en la misma regla.
    #[test]
    fn los_anchos_no_son_paleta() {
        let r = de("<maqueta><style>.box { border-width:1px; color:#FFFFFF }</style></maqueta>");
        assert!(r.contains("pub const BOX: u32 = 0x00FF_FFFF;"), "{r}");
        assert!(!r.contains("WIDTH"), "un ancho suelto no es una constante de tema:\n{r}");
    }

    /// [!] Un color de `:hover` es interaccion, no paleta.
    #[test]
    fn el_hover_no_entra_en_la_paleta() {
        let r = de(
            "<maqueta><style>.tecla { color:#111111 } .tecla:hover { color:#222222 }</style></maqueta>",
        );
        assert!(r.contains("0x0011_1111"), "{r}");
        assert!(!r.contains("0x0022_2222"), "el hover se colo en la paleta:\n{r}");
    }

    /// *** Y EL TEMA DE VERDAD, el que usa el escritorio.
    ///
    /// ** Este test es el que impide que el emisor y el fichero se separen: no
    /// comprueba un ejemplo inventado, comprueba **el fichero que manda**.
    #[test]
    fn el_tema_de_la_casa_sale_entero() {
        let src = include_str!("../../tema/tema.maqueta");
        let doc = bmo_maqueta_node::parse(src.as_bytes()).expect("tema.maqueta no compila");
        let r = paleta("toolchain/tools/maqueta/tema/tema.maqueta", &doc);
        // Las cinco tintas y las cuatro superficies que declara.
        for esperado in [
            "pub const INK: u32 = 0x00E6_EDF6;",
            "pub const INK_DIM: u32 = 0x008A_9BB4;",
            "pub const INK_OK: u32 = 0x007E_E787;",
            "pub const INK_BAD: u32 = 0x00FF_8A7A;",
            "pub const ACCENT: u32 = 0x0060_A5FA;",
            "pub const BOX_FONDO: u32 = 0x001E_2534;",
            "pub const BOX_BORDE: u32 = 0x0033_3D52;",
            "pub const FIELD_FONDO: u32 = 0x0016_1C28;",
            "pub const TASKBAR_FONDO: u32 = 0x000F_131D;",
            "pub const BG_TOP_FONDO: u32 = 0x001B_2233;",
        ] {
            assert!(r.contains(esperado), "falta `{esperado}` en:\n{r}");
        }
    }

    /// Un tema sin colores lo dice, en vez de emitir un fichero vacio que
    /// compila y no sirve.
    #[test]
    fn un_tema_sin_colores_se_queja_dentro_del_artefacto() {
        let r = de("<maqueta><style>.caja { width:10px }</style></maqueta>");
        assert!(r.contains("NINGUN color"), "{r}");
    }
}
