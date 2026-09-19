//! **El preprocesador de C++, que es el de C** -- paso 1 de `BRECHA.md`, la
//! mitad que faltaba (2026-09-18).
//!
//! === Por que no es "C++ lee el texto expandido" ===
//!
//! Las cabeceras del sistema de BMO (`<bmo/monton.h>`, `<stdio.h>`...) son C
//! **con sus cuerpos**: no hubo enlazador durante meses, asi que la cabecera ES
//! la implementacion. Y el parser de C++ es un subconjunto a proposito: no sabe
//! `sizeof`, `typedef` ni la mitad de lo que esas cabeceras traen. Leer el
//! texto expandido entero como C++ fallaria en la primera cabecera.
//!
//! Asi que se hace lo que hace `extern "C"` en cualquier compilador: **el C se
//! lee como C**.
//!
//!   1. el preprocesador de BMO C expande la unidad entera y marca que lineas
//!      vinieron de una cabecera DEL SISTEMA (`<...>`; `rangos_sistema`);
//!   2. esas lineas las analiza el frontend de C; el resto, el de C++. Las dos
//!      mitades conservan los numeros de linea del texto expandido (la otra
//!      mitad queda en blanco), asi que un error dice la misma linea que C;
//!   3. los dos arboles de C se juntan -- lo de las cabeceras DELANTE, porque
//!      declara antes de que se use -- y lo de las cabeceras queda como COPIA
//!      privada de la unidad (`Libc::Copia`), la misma regla que en C.
//!
//! Una cabecera PROPIA (`#include "mio.h"`) es C++ del que compila, y la lee
//! el parser de C++.

use std::path::Path;

use bmo_c_front::{ast as c, CStandard, Libc};

use crate::{descenso, parser, CppError};

/// El estandar con el que se leen las cabeceras de C: el mismo que usa BMO C
/// por defecto, para que una cabecera signifique lo mismo en los dos.
const ESTANDAR: CStandard = CStandard::DefaultC;

/// Una unidad de C++, preprocesada y bajada al arbol de C. `objeto` es el de
/// `descender_unidad`: si la unidad tiene que traer `main` o no.
pub fn a_c(source: &str, ruta: &Path, objeto: bool) -> Result<c::Program, CppError> {
    let (expandido, rangos) =
        bmo_c_front::preprocesar_con_rangos(source, ruta, ESTANDAR).map_err(de_c)?;
    let del_sistema = |l: usize| rangos.iter().any(|(a, b)| l >= *a && l <= *b);

    let (mut en_cpp, mut en_c) = (String::new(), String::new());
    for (i, linea) in expandido.lines().enumerate() {
        let (suya, otra) = if del_sistema(i + 1) { (&mut en_c, &mut en_cpp) } else { (&mut en_cpp, &mut en_c) };
        suya.push_str(linea);
        suya.push('\n');
        otra.push('\n');
    }

    let programa = parser::parse(&en_cpp)?;
    let salida = descenso::descender_unidad(&programa, objeto)?;
    let rasgos = bmo_c_front::StandardFeatures::load_standard(ESTANDAR);
    let mut cabeceras = if en_c.trim().is_empty() {
        None
    } else {
        let mut c = bmo_c_front::parse_with_features(&en_c, &rasgos).map_err(de_c)?;
        bmo_c_front::politica_libc(&mut c, &rangos, Libc::Copia);
        Some(c)
    };

    // ** `new` y `delete` traen el monton SIN pedirlo (2026-09-18). En C++ el
    // `operator new` esta declarado de forma implicita: obligar a escribir un
    // `#include` para usarlo haria a BMO C++ mas raro que C++. Entra como si
    // se hubiera incluido `<bmo/monton.h>` -- leido como C y como copia
    // privada -- y si la unidad ya lo trajo, no se duplica.
    let ya_esta = cabeceras.as_ref().map_or(false, |c| c.functions.iter().any(|f| f.name == "malloc"));
    if programa.usa_monton && !ya_esta {
        let (texto, r) = bmo_c_front::preprocesar_con_rangos("#include <bmo/monton.h>\n", ruta, ESTANDAR)
            .map_err(de_c)?;
        let mut m = bmo_c_front::parse_with_features(&texto, &rasgos).map_err(de_c)?;
        bmo_c_front::politica_libc(&mut m, &r, Libc::Copia);
        cabeceras = Some(match cabeceras {
            Some(mut c) => { juntar(&mut c, m); c }
            None => m,
        });
    }

    match cabeceras {
        None => Ok(salida),
        Some(mut c) => { juntar(&mut c, salida); Ok(c) }
    }
}

/// `detras` va DETRAS de `delante`: lo de las cabeceras declara antes de usarse.
fn juntar(delante: &mut c::Program, mut detras: c::Program) {
    delante.globals.append(&mut detras.globals);
    delante.functions.append(&mut detras.functions);
    delante.exported.append(&mut detras.exported);
    delante.disposiciones.extend(detras.disposiciones);
    delante.enlace.prototipos.append(&mut detras.enlace.prototipos);
    delante.enlace.estaticos.append(&mut detras.enlace.estaticos);
    delante.enlace.solo_externos.append(&mut detras.enlace.solo_externos);
}

fn de_c(e: bmo_c_front::CError) -> CppError {
    CppError::new(e.line, e.message)
}
