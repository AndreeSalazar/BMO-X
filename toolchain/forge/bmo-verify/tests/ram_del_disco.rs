//! **Cuanto de cada `.bex` desplegable se podria entregar SIN leerlo.**
//!
//! Hermana de `hashes_del_disco.rs`, y por el mismo motivo: una pregunta sobre
//! un fichero que se puede contestar en el anfitrion no vale encender la
//! maquina para contestarla.
//!
//! Lo que contesta, por fichero y en total:
//!
//! ```text
//!    no viaja     bytes deducibles (Bss) que ya no se transportan  -- escalon 0
//!    mapeable     bytes que el cargador podria MAPEAR              -- escalon 7
//!    copia        bytes que hay que leer si o si, y POR QUE
//! ```
//!
//! # Por que esta prueba no falla nunca, y aun asi sirve
//!
//! Porque hoy la respuesta es **cero mapeable**, y eso es correcto: el escritor
//! de BEF alinea los `file_offset` a 8 bytes y nadie le ha pedido congruencia de
//! pagina. Fallar aqui seria romper el build por una regla que todavia no se
//! puede cumplir.
//!
//! Lo que hace es **poner el numero en la pantalla**. El escalon 7 de
//! `docs/identidad/LA_RAM.md` esta marcado `XL` y sin cifra al lado; con esto pasa a tener
//! una: *"tantos bytes por arranque"*. Un escalon caro con un ahorro medido se
//! puede priorizar; uno caro sin cifra se aplaza para siempre.
//!
//! [!] No falla si no hay `staging`: es artefacto del build, no del repositorio.

use bmo_verify::ram::{auditar_ram, Transporte};
use std::path::{Path, PathBuf};

fn recoger(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            recoger(&p, out);
        } else if p.extension().map(|x| x == "bex").unwrap_or(false) {
            out.push(p);
        }
    }
}

/// **El censo de transporte de todo lo que se despliega.**
#[test]
fn cuanto_de_cada_bex_podria_no_leerse() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../Ultra_kernel_x86-64/staging/BMO-DATA");
    if !raiz.exists() {
        eprintln!("sin staging: no hay nada que medir (build.ps1 no ha corrido)");
        return;
    }
    let mut ficheros = Vec::new();
    recoger(&raiz, &mut ficheros);
    ficheros.sort();
    assert!(!ficheros.is_empty(), "hay staging pero ni un .bex dentro");

    let (mut t_no, mut t_map, mut t_cop) = (0u64, 0u64, 0u64);
    // Un motivo de ejemplo, para no repetir el mismo texto ochenta veces: si
    // todos los ficheros fallan por lo mismo, decirlo ochenta veces no informa
    // ochenta veces.
    let mut un_motivo = String::new();

    eprintln!("  {:16} {:>12} {:>12} {:>12}", "fichero", "no viaja", "mapeable", "copia");
    for f in &ficheros {
        let Ok(d) = std::fs::read(f) else { continue };
        let inf = auditar_ram(&d);
        if inf.filas.is_empty() {
            continue;
        }
        t_no += inf.no_viajan;
        t_map += inf.mapeables;
        t_cop += inf.copiados;
        if un_motivo.is_empty() {
            for fila in &inf.filas {
                if fila.transporte == Transporte::Copia && !fila.motivo.is_empty() {
                    un_motivo = fila.motivo.clone();
                    break;
                }
            }
        }
        let nombre = f.file_name().unwrap().to_string_lossy().to_string();
        eprintln!(
            "  {:16} {:>12} {:>12} {:>12}",
            nombre, inf.no_viajan, inf.mapeables, inf.copiados
        );
    }

    eprintln!("  {:16} {:>12} {:>12} {:>12}", "TOTAL", t_no, t_map, t_cop);
    if !un_motivo.is_empty() {
        eprintln!("  y por que se copia: {un_motivo}");
    }
    eprintln!(
        "  se leen hoy {} B por despliegue; mapeando serian {} B menos",
        t_map + t_cop,
        t_map
    );

    // ** LO UNICO QUE SE AFIRMA, porque es lo unico que hoy es cierto siempre.
    //
    // Si algun dia esto falla, es que el escritor empezo a alinear a pagina y el
    // escalon 7 dejo de ser teoria. Entonces se borra la asercion y se celebra.
    assert_eq!(
        t_map, 0,
        "hay {t_map} bytes mapeables: el escritor de BEF empezo a alinear a pagina. \
         Borra esta asercion, actualiza el escalon 7 de docs/identidad/LA_RAM.md, y celebra"
    );

    // Y esto si es una verdad estructural: si NADA se puede saltar, el escalon 0
    // no esta puesto -- y ese si esta hecho desde el 2026-08-09.
    assert!(
        t_no > 0,
        "ni un byte deducible en todo el staging: la seccion Bss dejo de emitirse"
    );
}
