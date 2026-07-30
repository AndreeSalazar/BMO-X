//! `bmo-ada-front` — el compilador de Ada, por la línea de órdenes.

use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut fuente: Option<String> = None;
    let mut salida: Option<PathBuf> = None;

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
            otro => fuente = Some(otro.to_string()),
        }
        i += 1;
    }

    let Some(ruta) = fuente else {
        eprintln!("uso: bmo-ada-front <fichero.adb> [-o salida.bex]");
        process::exit(2);
    };

    let texto = match std::fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: no puedo leer {ruta}: {e}");
            process::exit(1);
        }
    };

    let bytes = match bmo_ada_front::compilar(&texto) {
        Ok(b) => b,
        Err(e) => {
            // El error lleva su línea y su motivo. Un compilador que dice "no
            // se pudo" manda a leer código.
            eprintln!("{ruta}:{}", e);
            process::exit(1);
        }
    };

    let destino = salida.unwrap_or_else(|| {
        let mut p = PathBuf::from(&ruta);
        p.set_extension("bex");
        p
    });
    if let Some(padre) = destino.parent() {
        if !padre.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(padre);
        }
    }
    match std::fs::write(&destino, &bytes) {
        Ok(()) => println!("ok: wrote {} bytes -> {}", bytes.len(), destino.display()),
        Err(e) => {
            eprintln!("error: no puedo escribir {}: {e}", destino.display());
            process::exit(1);
        }
    }
}
