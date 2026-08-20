//! EL LINAJE: quien puede depender de quien, vigilado.
//!
//! ## Por que esto es un test y no un dibujo
//!
//! Peticion de Eddi, 2026-08-19: *"la jerarquia de abuelo, padre, hijo, nieto,
//! cada uno modular... INTI si o si tiene que tener piezas MUY indestructibles,
//! para no cambiar la pieza sino cambiarla de GOLPE cuando un dia falle"*.
//!
//! Una pieza solo se puede cambiar de golpe si **nadie la ha agarrado por
//! dentro**. Y eso no se consigue escribiendolo en un documento: los documentos
//! no impiden un `use`. Se consigue midiendo las flechas.
//!
//! ```text
//!    documento         dice como deberia ser
//!    este test         dice como ES, y falla si deja de serlo
//! ```
//!
//! ## La ley
//!
//! > **Un modulo solo puede depender de generaciones ANTERIORES.**
//! > Ni hacia arriba, ni hacia los lados.
//!
//! Hacia arriba es obvio por que no: seria un ciclo. **Hacia los lados es el
//! importante y el que nadie vigila**: dos hermanos que se llaman entre si son
//! una sola pieza con dos nombres, y el dia que haya que cambiar uno hay que
//! cambiar los dos.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Las generaciones de dentro del frontend, de mayor a menor edad.
///
/// El numero no es un orden de importancia: es **cuanto puede mirar**. El cero
/// no mira a nadie, y por eso es el unico que se puede tirar y reescribir sin
/// leer nada mas.
fn generaciones() -> HashMap<&'static str, u32> {
    let mut g = HashMap::new();

    // -- ABUELOS: no importan NADA del crate. Se prueban solos. ----------
    g.insert("aviso", 0);
    g.insert("palabras", 0);

    // -- PADRES: leen a los abuelos y nada mas. --------------------------
    g.insert("lexico", 1);
    g.insert("arquitectura", 1);
    // `cabina` traduce avisos y numeros a eventos. Mira a `aviso` y a nadie
    // mas: si mirase a `perfil` o a `ir` seria un quinto analisis disfrazado de
    // informe, y se los llevaria por delante el dia que se reescriban.
    g.insert("cabina", 1);

    // -- HIJOS -----------------------------------------------------------
    g.insert("arbol", 2);

    // -- NIETOS: los que aplican reglas sobre la forma. ------------------
    g.insert("sintaxis", 3);

    // -- BISNIETOS: analisis y descenso. Ninguno mira a otro. ------------
    g.insert("perfil", 4);
    g.insert("nombres", 4);
    g.insert("ir", 4);

    g
}

/// Los ficheros de `src/`, con el modulo al que pertenecen.
fn modulos() -> Vec<(String, PathBuf)> {
    fn anda(dir: &Path, modulo: &str, v: &mut Vec<(String, PathBuf)>) {
        for e in std::fs::read_dir(dir).expect("no puedo leer src/") {
            let p = e.expect("entrada rara").path();
            if p.is_dir() {
                let nombre = p.file_name().unwrap().to_string_lossy().to_string();
                anda(&p, &nombre, v);
            } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
                let nombre = p.file_stem().unwrap().to_string_lossy().to_string();
                // `lib.rs` es el cableado: no es una generacion, las junta.
                if nombre != "lib" {
                    v.push((modulo.to_string(), p));
                }
            }
        }
    }
    let mut v = Vec::new();
    anda(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), "", &mut v);
    v
}

/// A quien mira un fichero: los `crate::X` que escribe.
fn a_quien_mira(texto: &str) -> Vec<String> {
    let mut v = Vec::new();
    for linea in texto.lines() {
        let l = linea.trim();
        // Los comentarios hablan de otros modulos todo el rato, y hablar no es
        // depender. Solo cuenta un `use` de verdad.
        if !l.starts_with("use ") && !l.contains("crate::") {
            continue;
        }
        let mut resto = l;
        while let Some(i) = resto.find("crate::") {
            resto = &resto[i + 7..];
            let fin = resto
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(resto.len());
            let nombre = &resto[..fin];
            if !nombre.is_empty() {
                v.push(nombre.to_string());
            }
            resto = &resto[fin..];
        }
    }
    v
}

/// ** La ley: nadie mira hacia arriba ni hacia los lados.
#[test]
fn ningun_modulo_mira_a_su_generacion_ni_a_las_siguientes() {
    let g = generaciones();
    let mut culpables = Vec::new();

    for (modulo, ruta) in modulos() {
        // Las pruebas de un modulo pueden mirar lo que haga falta: prueban, no
        // se entregan. Es la misma frontera que separa `src/` de `tests/`.
        if ruta.file_stem().unwrap() == "pruebas" {
            continue;
        }
        let mia = match g.get(modulo.as_str()) {
            Some(n) => *n,
            None => continue,
        };
        let texto = std::fs::read_to_string(&ruta).expect("no puedo leer");

        for otro in a_quien_mira(&texto) {
            if otro == modulo {
                continue;
            }
            if let Some(suya) = g.get(otro.as_str()) {
                if *suya >= mia {
                    culpables.push(format!(
                        "{} (gen {}) mira a {} (gen {}) en {}",
                        modulo,
                        mia,
                        otro,
                        suya,
                        ruta.file_name().unwrap().to_string_lossy()
                    ));
                }
            }
        }
    }

    assert!(
        culpables.is_empty(),
        "el linaje se rompio -- una pieza dejo de poder cambiarse sola:\n  {}",
        culpables.join("\n  ")
    );
}

/// ** Los abuelos no miran a nadie, y por eso se pueden tirar y reescribir sin
/// leer una linea del resto.
///
/// Es la definicion operativa de "indestructible" que este proyecto usa: no que
/// no se rompa nunca, sino que **romperla no rompa nada mas**.
#[test]
fn los_abuelos_no_dependen_de_nadie() {
    let g = generaciones();
    let mut culpables = Vec::new();

    for (modulo, ruta) in modulos() {
        if ruta.file_stem().unwrap() == "pruebas" {
            continue;
        }
        if g.get(modulo.as_str()) != Some(&0) {
            continue;
        }
        let texto = std::fs::read_to_string(&ruta).expect("no puedo leer");
        for otro in a_quien_mira(&texto) {
            if otro != modulo && g.contains_key(otro.as_str()) {
                culpables.push(format!("{} mira a {}", modulo, otro));
            }
        }
    }

    assert!(
        culpables.is_empty(),
        "un abuelo agarro a otro modulo:\n  {}",
        culpables.join("\n  ")
    );
}

/// Y los bisnietos --los tres analisis-- **no se miran entre ellos**.
///
/// Es el caso que mas facil se cuela: `nombres` necesita algo que `perfil` ya
/// calculo, y llamarle parece gratis. No lo es: el dia que uno de los dos se
/// reescriba, el otro se va con el.
#[test]
fn los_tres_analisis_no_se_miran_entre_ellos() {
    let g = generaciones();
    let hermanos: Vec<&str> = g
        .iter()
        .filter(|(_, n)| **n == 4)
        .map(|(m, _)| *m)
        .collect();
    assert!(hermanos.len() >= 3, "deberia haber tres analisis");

    for (modulo, ruta) in modulos() {
        if ruta.file_stem().unwrap() == "pruebas" || !hermanos.contains(&modulo.as_str()) {
            continue;
        }
        let texto = std::fs::read_to_string(&ruta).expect("no puedo leer");
        for otro in a_quien_mira(&texto) {
            assert!(
                otro == modulo || !hermanos.contains(&otro.as_str()),
                "{} mira a su hermano {}: son dos piezas o una, no las dos cosas",
                modulo,
                otro
            );
        }
    }
}

/// El coste de cambiar una pieza, en un numero.
///
/// ** Es la pregunta de Eddi hecha medida: *cuantos ficheros hay que tocar para
/// sustituir esto?* Si es uno, la pieza es modular. Si son cinco, no lo es
/// aunque el documento diga que si.
#[test]
fn cambiar_un_modulo_no_arrastra_a_medio_compilador() {
    let g = generaciones();
    let todos = modulos();

    for (modulo, _) in &g {
        let mut quien_lo_mira = 0;
        for (otro, ruta) in &todos {
            if otro == modulo || ruta.file_stem().unwrap() == "pruebas" {
                continue;
            }
            let texto = std::fs::read_to_string(ruta).expect("no puedo leer");
            if a_quien_mira(&texto).iter().any(|x| x == modulo) {
                quien_lo_mira += 1;
            }
        }
        // `aviso` lo mira todo el mundo a proposito: es el contrato del
        // mensaje, y un contrato compartido es lo contrario de un acoplamiento.
        // El resto no puede pasar de la mitad del compilador.
        let tope = if *modulo == "aviso" { 99 } else { 6 };
        assert!(
            quien_lo_mira <= tope,
            "{} lo miran {} ficheros: cambiarlo ya no es cambiar una pieza",
            modulo,
            quien_lo_mira
        );
    }
}
