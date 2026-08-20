//! El censo, barrido de verdad.
//!
//! ## Que puede y que no puede comprobar esto todavia
//!
//! `CENSO.md` declara 38 sondas con su veredicto escrito por delante. La
//! mayoria de esos veredictos son de fases que **no existen aun** (`E0030` es
//! del analisis de nombres, `E0080` del de tareas). Comprobarlos hoy seria
//! fingir.
//!
//! Lo que si se puede comprobar hoy, y es exactamente lo que hace este fichero:
//!
//! 1. **Que las 38 sondas se pueden leer**, y que ninguna lleva un fallo de
//!    escritura escondido -- margenes torcidos, comillas sin cerrar, signos de
//!    otro lenguaje.
//! 2. **Que las que declaran un veredicto LEXICO lo cumplen ya.**
//!
//! El primer punto ya se gano el sitio antes de correr: las 38 sondas estaban
//! escritas con **tres** espacios de sangria y la gramatica dice **cuatro**. El
//! documento y el corpus llevaban dos dias sin estar de acuerdo, y nadie lo
//! habria visto leyendo.

use std::path::{Path, PathBuf};

use bmo_inti_front::{barrer, Clase};

fn carpeta() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("censo")
}

fn sondas() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = std::fs::read_dir(carpeta())
        .expect("no encuentro censo/")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "inti").unwrap_or(false))
        .map(|p| {
            let nombre = p.file_stem().unwrap().to_string_lossy().to_string();
            let texto = std::fs::read_to_string(&p).expect("no puedo leer la sonda");
            (nombre, texto)
        })
        .collect();
    v.sort();
    v
}

/// El veredicto que la propia sonda declara en su primera linea.
fn veredicto(texto: &str) -> String {
    let primera = texto.lines().next().unwrap_or("");
    match primera.split("espera:").nth(1) {
        Some(t) => t.trim().to_string(),
        None => String::new(),
    }
}

#[test]
fn el_censo_tiene_las_sondas_que_dice() {
    assert_eq!(sondas().len(), 38, "el numero del censo y el de la carpeta");
}

/// Cada sonda declara su veredicto en la primera linea, para que la sonda y su
/// expectativa no se puedan separar.
#[test]
fn todas_las_sondas_declaran_su_veredicto() {
    for (nombre, texto) in sondas() {
        assert!(
            !veredicto(&texto).is_empty(),
            "{} no dice que espera en su primera linea",
            nombre
        );
    }
}

/// Ninguna sonda lleva un fallo de escritura escondido.
///
/// Se excluyen las dos que existen justamente para llevarlo: `s03` trae un
/// tabulador y `s05` una comilla simple. Que la lista de excepciones sea
/// EXACTA importa -- si manana otra sonda empieza a fallar en el barrido,
/// tiene que romper este test y no colarse en la excepcion de al lado.
#[test]
fn ninguna_sonda_lleva_un_fallo_de_escritura() {
    const LO_LLEVAN_A_PROPOSITO: &[(&str, &str)] =
        &[("s03_tabulador", "E0010"), ("s05_comilla_simple", "E0011")];

    for (nombre, texto) in sondas() {
        let c = barrer(&texto);
        let codigos = c.codigos();

        match LO_LLEVAN_A_PROPOSITO.iter().find(|(n, _)| *n == nombre) {
            Some((_, esperado)) => assert!(
                codigos.contains(esperado),
                "{} tenia que dar {} y dio {:?}",
                nombre,
                esperado,
                codigos
            ),
            None => assert!(
                codigos.is_empty(),
                "{} deberia barrerse limpia y dio {:?}\n{}",
                nombre,
                codigos,
                c.pintar(&format!("{}.inti", nombre))
            ),
        }
    }
}

/// Toda sonda empieza por `perfil`, porque el lenguaje no tiene perfil por
/// defecto -- salvo `s02`, que existe para probar justamente que falta.
#[test]
fn toda_sonda_declara_su_perfil() {
    for (nombre, texto) in sondas() {
        if nombre == "s02_sin_perfil" {
            continue;
        }
        let piezas = barrer(&texto).valor;
        let primera = piezas
            .iter()
            .find(|p| !matches!(p.clase, Clase::FinLinea | Clase::Sangra | Clase::Desangra))
            .expect("una sonda vacia");
        assert!(
            primera.es(bmo_inti_front::Simbolo::Perfil),
            "{} no empieza por `perfil`, empieza por {}",
            nombre,
            primera.como_se_llama()
        );
    }
}

/// Las dos familias del corpus tienen que estar representadas: si algun dia se
/// borran todas las sondas de `llano`, el lenguaje habria dejado de tener dos
/// perfiles sin que nadie lo dijera.
#[test]
fn el_corpus_cubre_los_dos_perfiles() {
    let mut llano = 0;
    let mut pleno = 0;
    for (_, texto) in sondas() {
        let piezas = barrer(&texto).valor;
        for (i, p) in piezas.iter().enumerate() {
            if p.es(bmo_inti_front::Simbolo::Perfil) {
                match piezas.get(i + 1) {
                    Some(q) if q.es(bmo_inti_front::Simbolo::Llano) => llano += 1,
                    Some(q) if q.es(bmo_inti_front::Simbolo::Pleno) => pleno += 1,
                    _ => {}
                }
                break;
            }
        }
    }
    assert!(llano >= 5, "pocas sondas de `llano`: {}", llano);
    assert!(pleno >= 10, "pocas sondas de `pleno`: {}", pleno);
}

/// Las sondas, pasadas por la GRAMATICA y no solo por el barrido.
///
/// Aqui solo se comprueban las que declaran `COMPILA`: las que esperan un
/// codigo de una fase que aun no existe (`E0030` es del analisis de nombres)
/// no se pueden juzgar todavia, y fingir que si seria peor que no mirarlas.
#[test]
fn las_sondas_que_dicen_compila_se_leen_enteras() {
    for (nombre, texto) in sondas() {
        let v = veredicto(&texto);
        if !v.starts_with("COMPILA") {
            continue;
        }
        let c = bmo_inti_front::leer(&texto);
        assert!(
            !c.hay_errores(),
            "{} dice COMPILA y no se lee:\n{}",
            nombre,
            c.pintar(&format!("{}.inti", nombre))
        );
    }
}

/// Las sondas de PERFIL, comprobadas de verdad.
///
/// Estas ya no son promesas: `p02`, `p03`, `p04` y `p07` declaran un codigo que
/// el analisis de perfiles sabe dar hoy, asi que se exige. Las demas siguen
/// esperando a su fase.
#[test]
fn las_sondas_de_perfil_dan_su_codigo() {
    const AHORA_SE_PUEDEN: &[(&str, &str)] = &[
        ("p02_llano_sin_lista", "E0070"),
        ("p03_llano_sin_numero", "E0020"),
        ("p04_crudo_en_pleno", "E0071"),
        ("p07_puerto_sin_crudo", "E0072"),
    ];

    for (nombre, esperado) in AHORA_SE_PUEDEN {
        let (_, texto) = sondas()
            .into_iter()
            .find(|(n, _)| n == nombre)
            .unwrap_or_else(|| panic!("falta la sonda {}", nombre));

        let c = bmo_inti_front::comprobar(&texto);
        assert!(
            c.codigos().contains(esperado),
            "{} tenia que dar {} y dio {:?}\n{}",
            nombre,
            esperado,
            c.codigos(),
            c.pintar(&format!("{}.inti", nombre))
        );
    }
}

/// Y las que dicen COMPILA siguen sin dar ni un aviso al pasar por el perfil.
#[test]
fn las_sondas_que_compilan_pasan_el_perfil() {
    for (nombre, texto) in sondas() {
        if !veredicto(&texto).starts_with("COMPILA") {
            continue;
        }
        let c = bmo_inti_front::comprobar(&texto);
        assert!(
            !c.hay_errores(),
            "{} dice COMPILA y el perfil la rechaza:\n{}",
            nombre,
            c.pintar(&format!("{}.inti", nombre))
        );
    }
}
