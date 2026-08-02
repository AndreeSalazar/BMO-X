//! `bmo-cpp-front` — compila C++ a BEF.
//!
//! Antes imprimía los contadores de un `IrModule` que nadie consumía: decía
//! "OK: compiled" sin haber producido un solo byte ejecutable. Ahora escribe
//! el `.bef` o falla diciendo por qué.

use std::env;
use std::fs;
use std::path::PathBuf;
use bmo_cpp_front::compile_source_to_bef;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: bmo-cpp-front <fichero.cpp> [salida.bef]");
        std::process::exit(1);
    }

    let entrada = PathBuf::from(&args[1]);
    let fuente = match fs::read_to_string(&entrada) {
        Ok(s) => s,
        Err(e) => { eprintln!("no se pudo leer {}: {e}", entrada.display()); std::process::exit(1); }
    };

    let bef = match compile_source_to_bef(&fuente) {
        Ok(b) => b,
        Err(e) => {
            if e.line > 0 { eprintln!("error en la linea {}: {}", e.line, e.message); }
            else { eprintln!("error: {}", e.message); }
            std::process::exit(1);
        }
    };

    let salida = if args.len() > 2 {
        PathBuf::from(&args[2])
    } else {
        entrada.with_extension("bef")
    };

    match fs::write(&salida, &bef) {
        Ok(()) => println!("{} — {} bytes", salida.display(), bef.len()),
        Err(e) => { eprintln!("no se pudo escribir {}: {e}", salida.display()); std::process::exit(1); }
    }
}
