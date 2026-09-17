//! `bmo-enlazar` -- junta objetos (`.bo`) en un ejecutable (`.bex`).
//!
//! ```text
//!   bmo-enlazar -o programa.bex uno.bo dos.bo
//! ```
//!
//! El orden de los objetos es el que se escribe, y el mismo mandato da los
//! mismos bytes: ver `enlazar_dos_veces_da_los_mismos_bytes`.

use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut salida: Option<PathBuf> = None;
    let mut entradas: Vec<PathBuf> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                i += 1;
                match args.get(i) {
                    Some(p) => salida = Some(PathBuf::from(p)),
                    None => {
                        eprintln!("error: -o necesita una ruta");
                        process::exit(2);
                    }
                }
            }
            otro => entradas.push(PathBuf::from(otro)),
        }
        i += 1;
    }

    if entradas.is_empty() {
        eprintln!("uso: bmo-enlazar -o programa.bex uno.bo [dos.bo ...]");
        process::exit(2);
    }

    let mut unidades = Vec::with_capacity(entradas.len());
    for ruta in &entradas {
        match std::fs::read(ruta) {
            Ok(b) => unidades.push((ruta.display().to_string(), b)),
            Err(e) => {
                // Un objeto que no se puede leer para el enlace entero: media
                // imagen enlazada es peor que ninguna (regla 1b).
                eprintln!("error: no puedo leer {}: {e}", ruta.display());
                process::exit(1);
            }
        }
    }

    let bex = match bmo_enlazar::enlazar(&unidades) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(1);
        }
    };

    let destino = salida.unwrap_or_else(|| entradas[0].with_extension("bex"));
    match std::fs::write(&destino, &bex) {
        Ok(()) => println!("ok: wrote {} bytes -> {}", bex.len(), destino.display()),
        Err(e) => {
            eprintln!("error: no puedo escribir {}: {e}", destino.display());
            process::exit(1);
        }
    }
}
