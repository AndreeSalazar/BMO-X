//! **La seccion `Symbols`: que funcion vive en cada offset del `.bex`.**
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes viven en `tests/mod.rs`.
//!
//! # Que se comprueba aqui, y por que estas filas y no otras
//!
//! El compilador siempre supo donde empieza cada funcion --`function_offsets`
//! lo lleva dentro para resolver las llamadas-- y desde el 2026-08-13 lo dice
//! por la consola con `--map`. Escribirlo en el binario es lo que cierra el
//! circuito: **el que tiene el `.bex` tiene los nombres**, sin recompilar.
//!
//! Asi que las filas no preguntan "existe la seccion" --eso lo diria un `assert`
//! de una linea-- sino las cuatro cosas que la hacen util:
//!
//! 1. que los offsets sean **los mismos** que resuelven las llamadas,
//! 2. que el tamano permita decir *"aqui NO hay ninguna funcion"*,
//! 3. y que un `rip` de verdad se traduzca a un nombre.
//!
//! No se comprueba aqui que la seccion **no se cargue** --y no cuesta una
//! pagina por binario-- porque `bmo-abi` ya tiene esa fila:
//! `lo_que_no_se_carga_no_paga_la_alineacion`. Dos pruebas de lo mismo en dos
//! crates es la forma de que un dia digan cosas distintas.

use super::*;
use bmo_abi::bef::sections::{SectionEntry, SectionKind};
use bmo_abi::bef::symbols::{name_hash, Symbol};

/// Los bytes de una seccion por su tipo, o `None` si no esta.
fn seccion(bef: &[u8], kind: SectionKind) -> Option<&[u8]> {
    let hdr = unsafe { &*(bef.as_ptr() as *const bmo_abi::bef::header::BefHeader) };
    let tabla = hdr.section_table_offset as usize;
    for i in 0..hdr.section_count as usize {
        let e = tabla + i * SectionEntry::SIZE;
        if bef[e] == kind as u8 {
            let off = u64::from_le_bytes(bef[e + 8..e + 16].try_into().unwrap()) as usize;
            let sz = u64::from_le_bytes(bef[e + 16..e + 24].try_into().unwrap()) as usize;
            return Some(&bef[off..off + sz]);
        }
    }
    None
}

/// Los simbolos de un `.bex`, ya emparejados con su nombre.
fn simbolos(bef: &[u8]) -> Vec<(String, u64, u64)> {
    let datos = seccion(bef, SectionKind::Symbols).expect("no hay seccion Symbols");
    // ** La CABECERA dice cuantas entradas hay. La primera version de esta
    // prueba lo adivinaba --contaba entradas mientras el campo `kind` valiera
    // `Function`-- y por eso daba verde con un binario que `bmo-verify`
    // rechazaba: el validador contaba de otra forma. Adivinar el limite es
    // exactamente el fallo que `TablaCadenas` vino a cerrar.
    let (n, cadenas_en) = bmo_abi::bef::sections::TablaCadenas::leer(datos, Symbol::SIZE)
        .expect("la cabecera declara mas entradas de las que caben");
    let cadenas = &datos[cadenas_en..];
    let base = bmo_abi::bef::sections::TablaCadenas::SIZE;
    let mut v = Vec::new();
    for i in 0..n {
        let e = base + i * Symbol::SIZE;
        let name_off = u32::from_le_bytes(datos[e..e + 4].try_into().unwrap()) as usize;
        let hash = u32::from_le_bytes(datos[e + 4..e + 8].try_into().unwrap());
        let addr = u64::from_le_bytes(datos[e + 8..e + 16].try_into().unwrap());
        let size = u64::from_le_bytes(datos[e + 16..e + 24].try_into().unwrap());
        let fin = cadenas[name_off..].iter().position(|&b| b == 0).unwrap();
        let nombre = core::str::from_utf8(&cadenas[name_off..name_off + fin])
            .unwrap()
            .to_string();
        assert_eq!(hash, name_hash(&nombre), "el hash guardado no es el del nombre");
        v.push((nombre, addr, size));
    }
    v
}

const TRES: &str = "
int uno(int a) { return a + 1; }
int dos(int a) { return uno(a) * 2; }
int main() { return dos(3); }
";

#[test]
fn el_bex_lleva_el_nombre_de_cada_funcion() {
    let bef = compile_source_to_bef(TRES).unwrap();
    let s = simbolos(&bef);
    let nombres: Vec<&str> = s.iter().map(|(n, _, _)| n.as_str()).collect();
    for esperado in ["uno", "dos", "main"] {
        assert!(nombres.contains(&esperado), "falta {}: {:?}", esperado, nombres);
    }
}

/// ** LA FILA QUE IMPIDE DOS VERDADES.
///
/// Los offsets de la seccion tienen que ser **los mismos** que el compilador usa
/// para resolver las llamadas. Si se calcularan aparte, el binario podria decir
/// que `main` esta en un sitio mientras los `call` saltan a otro -- y entonces
/// la tabla de simbolos seria una segunda opinion, que es peor que ninguna.
#[test]
fn los_offsets_son_los_MISMOS_que_resuelven_las_llamadas() {
    let programa = crate::parse(TRES).unwrap();
    let mapa = crate::codegen::function_map(&programa).unwrap();
    let bef = compile_source_to_bef(TRES).unwrap();

    for (nombre, addr, _) in simbolos(&bef) {
        let del_mapa = mapa
            .iter()
            .find(|(_, n)| *n == nombre)
            .unwrap_or_else(|| panic!("{} no esta en el mapa", nombre));
        assert_eq!(
            addr, del_mapa.0 as u64,
            "{}: la seccion dice {:#x} y las llamadas van a {:#x}",
            nombre, addr, del_mapa.0
        );
    }
}

/// ** EL TAMANO, que es lo que `--map` no daba.
///
/// Con solo los inicios, un `rip` entre dos funciones se atribuye a la de
/// arriba aunque caiga fuera. Con el tamano se puede contestar *"esa direccion
/// no esta en ninguna funcion"*, que es una respuesta distinta y correcta.
#[test]
fn el_tamano_permite_decir_que_una_direccion_NO_es_de_nadie() {
    let bef = compile_source_to_bef(TRES).unwrap();
    let s = simbolos(&bef);

    for (nombre, _, size) in &s {
        assert!(*size > 0, "{} tiene tamano cero", nombre);
    }

    // Nadie se solapa con nadie: cada funcion acaba antes de que empiece la
    // siguiente. Si esto falla, los tamanos son ficcion.
    let mut orden: Vec<_> = s.iter().map(|(n, a, t)| (*a, *t, n)).collect();
    orden.sort();
    for par in orden.windows(2) {
        let (a0, t0, n0) = &par[0];
        let (a1, _, n1) = &par[1];
        assert!(a0 + t0 <= *a1, "{} se solapa con {}", n0, n1);
    }
}

/// Traducir un `rip` a `funcion+desplazamiento`, que es para lo que existe todo
/// esto. Se hace con la tabla y nada mas -- sin fuente y sin recompilar.
#[test]
fn un_rip_se_traduce_a_nombre_mas_desplazamiento() {
    let bef = compile_source_to_bef(TRES).unwrap();
    let s = simbolos(&bef);
    let (_, dos_addr, dos_size) = s.iter().find(|(n, _, _)| n == "dos").unwrap().clone();

    // Un `rip` a mitad de `dos`, como el que trae una autopsia.
    let rip = dos_addr + dos_size / 2;
    let hallado = s.iter().find(|(_, a, t)| rip >= *a && rip < a + t);
    let (nombre, base, _) = hallado.expect("el rip tiene que caer en alguna funcion");
    assert_eq!(nombre, "dos");
    assert_eq!(rip - base, dos_size / 2);
}
