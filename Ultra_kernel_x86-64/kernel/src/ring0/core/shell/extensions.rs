//! **`ext` -- que ofrece este silicio y que coge BMO.**
//!
//! Grupo 1 del shell, como `hardware`: solo PREGUNTA. Si se equivoca da un
//! numero raro y nada mas.
//!
//! # Por que un fichero propio y no una seccion de `hardware.rs`
//!
//! Porque `hardware.rs` son ya 899 lineas y es el fichero del shell **que mas
//! crece** -- lo dice su propia cabecera. Meter aqui un censo de treinta y seis
//! filas lo empujaria a mil por un trabajo que no comparte nada con `disk`,
//! `net` ni `mem`: aquellos preguntan por un APARATO, este por el CONJUNTO DE
//! INSTRUCCIONES.
//!
//! Y son dos comandos y no uno a proposito:
//!
//! ```text
//!    cpu    el ESTADO extendido -- que registros pierde un cambio de contexto
//!    ext    el CONJUNTO DE INSTRUCCIONES -- que sabe hacer y que le pedimos
//! ```
//!
//! # Como se lee la tabla
//!
//! El censo vive en `cpu_vendor::features`, que es quien sabe. Aqui solo se
//! pinta, y se pinta en tres tramos porque **el producto de este comando no es
//! la lista: es la resta**.
//!
//! 1. Los grupos, compactos -- de un vistazo, que hay.
//! 2. **Lo que hay y no se coge, con lo que se ganaria.** Esto es el producto.
//! 3. Los CONFLICTOS, que tienen que ser cero.

use super::super::dashboard::dashboard_log_color;
use super::ui::{row, L, SH_TITLE, SH_VALUE};
use crate::ring0::cpu_vendor::features::{self, Group, Use};

/// Los ocho grupos, en el orden en que se pintan. Es el mismo orden que declara
/// `Feat::group`, escrito aqui porque un enum no se puede recorrer solo.
const GRUPOS: [Group; 8] = [
    Group::Vector,
    Group::Bits,
    Group::Random,
    Group::Crypto,
    Group::State,
    Group::Memory,
    Group::Time,
    Group::Guard,
];

fn linea(l: &L) {
    dashboard_log_color(l.as_str(), SH_VALUE);
    crate::ring0::dev::console::serial_write(l.as_str());
    crate::ring0::dev::console::serial_write("\n");
}

/// `ext` -- el censo de extensiones.
pub(crate) fn shell_ext() {
    let c = features::censar();

    dashboard_log_color("== EXTENSIONES : que ofrece el silicio y que coge BMO ==", SH_TITLE);
    row("leyenda", |l| l.txt("NOMBRE* lo usa   NOMBRE hay y no se usa   (NOMBRE) no hay   !NOMBRE CONFLICTO"));

    // -- 1. los grupos, compactos --------------------------------------
    for g in GRUPOS {
        let mut l = L::new();
        l.txt("  ");
        l.txt(g.title());
        l.col(30);
        for f in c.filas.iter() {
            if f.feat.group() != g {
                continue;
            }
            if f.conflicto() {
                l.txt("!");
                l.txt(f.feat.name());
            } else if !f.hay {
                l.txt("(");
                l.txt(f.feat.name());
                l.txt(")");
            } else {
                l.txt(f.feat.name());
                if f.uso.is_yes() {
                    l.txt("*");
                }
            }
            l.txt(" ");
        }
        linea(&l);
    }

    // -- 2. lo que hay y NO se coge. EL PRODUCTO ------------------------
    //
    // Va con su motivo al lado porque una lista de nombres no decide nada: lo
    // que decide es "que me daria". Sin esa columna esto seria trivia.
    dashboard_log_color("-- lo que hay y no se coge, y lo que daria --", SH_TITLE);
    for f in c.filas.iter() {
        if !f.desaprovechado() {
            continue;
        }
        let mut l = L::new();
        l.txt("  ");
        l.txt(f.feat.name());
        l.col(18);
        l.txt(f.uso.nota());
        linea(&l);
    }

    // -- 3. lo que SI se coge, con su sitio -----------------------------
    //
    // Es la columna que nadie puede comprobar a maquina --es prosa contra el
    // arbol-- asi que se pinta entera para que sea comprobable a ojo. La regla
    // esta escrita en `usage.rs`: un `USA` sin sitio es un `USA` que miente.
    dashboard_log_color("-- lo que si se coge, y donde --", SH_TITLE);
    for f in c.filas.iter() {
        if !f.uso.is_yes() {
            continue;
        }
        let mut l = L::new();
        l.txt("  ");
        l.txt(f.feat.name());
        l.col(18);
        l.txt(f.uso.nota());
        linea(&l);
    }

    // -- 4. los CONFLICTOS. La aguja gigante ----------------------------
    //
    // Usado y no presente no es una curiosidad: es una instruccion que va a
    // dar `#UD` en esta maquina. Se dice aunque sea cero, porque un cero
    // impreso es una comprobacion hecha y un hueco es una pregunta sin hacer.
    if c.conflictos > 0 {
        dashboard_log_color("!! CONFLICTO: BMO usa algo que este CPU NO declara !!", SH_TITLE);
        for f in c.filas.iter() {
            if !f.conflicto() {
                continue;
            }
            let mut l = L::new();
            l.txt("  !! ");
            l.txt(f.feat.name());
            l.col(18);
            l.txt(f.uso.nota());
            linea(&l);
        }
    }

    // -- el resumen va AL FINAL, y no es estetica -----------------------
    //
    // El panel es un log rodante: lo que sobrevive en la pantalla --y por tanto
    // en la foto-- es el final. La cifra que resume tiene que estar ahi.
    row("resumen", |l| {
        l.txt("hay ");
        l.dec(c.hay as u64);
        l.txt(" de ");
        l.dec(features::ALL.len() as u64);
        l.txt("   las usa ");
        l.dec(c.usadas as u64);
    });
    // ** LOS CUATRO QUE TIENEN QUE SER CERO, y se imprimen aunque lo sean.
    //
    // Son los `#[test]` que este modulo NO PUEDE tener: `cargo test` no
    // construye el crate del kernel, asi que la comprobacion baja a ser un
    // contador que se mira. Ver la cabecera de `features::Censo`.
    row("tiene que ser 0", |l| {
        l.txt("conflictos ");
        l.dec(c.conflictos as u64);
        l.txt("   mudas ");
        l.dec(c.mudas as u64);
        l.txt("   repetidas ");
        l.dec(c.repetidas as u64);
        l.txt("   sin sitio ");
        l.dec(c.sin_sitio as u64);
    });
}

/// El resumen en UNA linea, para el arranque.
///
/// Se separa de `shell_ext` porque el arranque no puede gastar cuarenta lineas
/// del panel, y porque la unica cifra que el arranque necesita vigilar es la
/// que tiene que ser cero. El censo completo se pide a mano.
pub(crate) fn aviso_de_arranque() {
    let c = features::censar();
    if c.averias() == 0 {
        return;
    }
    crate::ring0::cabina::warn(
        "cpu",
        "el censo de extensiones tiene averias: escribe ext",
        c.averias() as u64,
    );
}
