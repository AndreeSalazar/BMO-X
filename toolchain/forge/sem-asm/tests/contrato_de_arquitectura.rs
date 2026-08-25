//! **Toda carpeta de `tables/arch/` cumple el contrato, o no es una
//! arquitectura.**
//!
//! # Por que esto es un TEST y no un guardian de `build.ps1`
//!
//! Porque el mismo dia que se escribio, el sello de L6a dejo anotado que
//! `build.ps1` lleva **cinco entradas suyas** en la lista de subidas y que cada
//! guardian nuevo lo engorda:
//!
//! > *"El siguiente guardian NO se anade: primero se parte este fichero."*
//!
//! ** Respetar una regla propia el mismo dia que se escribe es lo unico que
//! hace que sirva. Y ademas aqui queda mejor: `bmo-sem-asm` es el crate que
//! LEE esas tablas, asi que la comprobacion vive con su dueno y entra en el
//! banco sin tocar el arranque.
//!
//! # Lo que comprueba, y lo que no puede comprobar
//!
//! Comprueba **la forma**: que esten los cuatro ficheros, que `[meta] isa`
//! diga el nombre de su carpeta, y que `abi.toml` traiga los campos sin los
//! cuales un emisor no puede ni empezar.
//!
//! [!] NO comprueba que los bytes sean correctos. Eso no lo puede decir un
//! test: lo dice el silicio. Ver `tables/arch/CONTRATO.md`.

use std::path::{Path, PathBuf};

fn arch() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tables").join("arch")
}

/// Las carpetas de `arch/`, que son las arquitecturas declaradas.
fn arquitecturas() -> Vec<(String, PathBuf)> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(arch()).expect("no hay tables/arch/") {
        let e = e.expect("entrada ilegible");
        if e.path().is_dir() {
            v.push((e.file_name().to_string_lossy().into_owned(), e.path()));
        }
    }
    v.sort();
    v
}

/// *** LOS CUATRO FICHEROS. Ver `CONTRATO.md`: los cuatro, o no es una
/// arquitectura.
///
/// ** Faltar uno no da un fallo bonito: da un emisor que compila y elige mal.
/// Sin `abi.toml` no se sabe que registro lleva el primer argumento, y sin eso
/// una llamada pasa basura -- que no revienta, **calcula otra cosa**.
#[test]
fn cada_arquitectura_trae_sus_cuatro_tablas() {
    let archs = arquitecturas();
    assert!(!archs.is_empty(), "no hay ninguna arquitectura declarada");
    for (nombre, dir) in &archs {
        for fichero in ["instructions.toml", "intrinsics.toml", "abi.toml", "inti.toml"] {
            assert!(
                dir.join(fichero).is_file(),
                "*** a la arquitectura `{nombre}` le falta `{fichero}`. Ver tables/arch/CONTRATO.md"
            );
        }
    }
}

/// **La tabla dice ser de la arquitectura de su carpeta.**
///
/// *** Una tabla que miente es PEOR que una que falta: la que falta no compila,
/// y la que miente **emite**. Copiar `arch/x86_64/` a `arch/riscv64/` y olvidar
/// esta linea da un compilador que jura tener un backend nuevo y escupe bytes
/// de x86.
#[test]
fn el_meta_isa_coincide_con_la_carpeta() {
    for (nombre, dir) in arquitecturas() {
        let t = std::fs::read_to_string(dir.join("intrinsics.toml")).unwrap();
        let declarada = t
            .lines()
            .find_map(|l| l.trim().strip_prefix("isa").and_then(|r| r.split('"').nth(1)))
            .unwrap_or_else(|| panic!("`{nombre}/intrinsics.toml` no declara `[meta] isa`"));
        assert_eq!(
            declarada, nombre,
            "*** `{nombre}/intrinsics.toml` dice ser de `{declarada}`"
        );
    }
}

/// **Los campos sin los que un emisor no puede ni empezar.**
///
/// ** No es la lista entera del contrato: es la que decide si una LLAMADA es
/// correcta. Que registros llevan los argumentos, donde vuelve el resultado,
/// cuales hay que devolver intactos y con que instruccion se entra al kernel.
///
/// [!] `callee_saved` esta en esta lista por un motivo concreto y escrito en
/// `CONTRATO.md` seccion 3: hoy el emisor de x86-64 lleva su propia copia
/// (`marco.rs`, `const RESPALDO`) y esta tabla es la otra. Dos sitios con la
/// misma verdad -- y este test es lo que impide que el de la tabla desaparezca
/// mientras nadie mira.
#[test]
fn el_abi_trae_lo_que_decide_una_llamada() {
    for (nombre, dir) in arquitecturas() {
        let t = std::fs::read_to_string(dir.join("abi.toml")).unwrap();
        for campo in [
            "arg_regs",
            "ret_reg",
            "callee_saved",
            "stack_align",
            "nr_reg",
            "instruction",
        ] {
            assert!(
                t.lines().any(|l| l.trim_start().starts_with(campo)),
                "*** `{nombre}/abi.toml` no declara `{campo}`. Ver tables/arch/CONTRATO.md"
            );
        }
    }
}

/// **`usa <nombre>` tiene que existir para cada arquitectura declarada.**
///
/// Es la idea de Eddi del 2026-08-19: la ISA entra por el PRINCIPIO del fichero
/// como una libreria, no por el final como una suposicion del emisor.
///
/// ** Y el nombre del modulo ES la declaracion de que ese fichero no es
/// portable. Por eso `inti.toml` no puede faltar aunque un chip nuevo no
/// tuviera ni un intrinseco util: el dia que lo tenga, el sitio ya existe.
#[test]
fn inti_puede_usar_cada_arquitectura() {
    for (nombre, dir) in arquitecturas() {
        let t = std::fs::read_to_string(dir.join("inti.toml")).unwrap();
        assert!(
            t.contains(&format!("usa {nombre}")),
            "*** `{nombre}/inti.toml` no documenta `usa {nombre}`, que es como se invoca"
        );
    }
}

/// *** MIENTRAS HAYA UNA SOLA ARQUITECTURA, ESTO ES UNA PROMESA.
///
/// El contrato dice que un chip nuevo es una carpeta de tablas y una puerta.
/// Con una sola carpeta esa frase **no se puede comprobar**: puede haber
/// supuestos de x86-64 repartidos por sitios que nadie mira, y todos los tests
/// de arriba pasarian igual.
///
/// ** Este caso existe para que ese hecho tenga un sitio donde estar escrito y
/// no se pierda en un documento que nadie abre. El dia que aparezca la segunda
/// carpeta, esto empieza a fallar -- y ese fallo es el recordatorio de leer
/// `CONTRATO.md` seccion 3 y sacar las dos tablas que aun viven en Rust.
#[test]
fn el_contrato_todavia_no_esta_probado_por_un_segundo_chip() {
    let cuantas = arquitecturas().len();
    assert_eq!(
        cuantas, 1,
        "*** hay {cuantas} arquitecturas: el contrato ya se puede COMPROBAR.\n\
         Toca sacar `RESPALDO` de marco.rs y la seleccion de operaciones.rs a\n\
         tablas -- ver tables/arch/CONTRATO.md seccion 3 -- y despues cambiar\n\
         este test por el que compare las dos."
    );
}
