//! `bmo-cpp-front` -- compila C++ a BEF.
//!
//! Antes imprimia los contadores de un `IrModule` que nadie consumia: decia
//! "OK: compiled" sin haber producido un solo byte ejecutable. Ahora escribe
//! el `.bef` o falla diciendo por que.

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

                // -- * EL GATE, ANTES DE ESCRIBIR --------------------------
            //
            // `bmo-verify` es el "unico checkpoint comun" de la filosofia: el
            // papel de seguridad que tendria un IR central, pero como CONTRATO
            // -- cada lenguaje emite su BEF por su cuenta y el verificador lo
            // revisa por separado. Hasta hoy no lo llamaba ningun frontend.
            //
            // Va ANTES del `write` a proposito: verificar despues dejaria un
            // fichero malo en el disco con un mensaje al lado, y quien lo
            // encuentre manana vera el `.bex`, no el mensaje.
            if let bmo_verify::Verdict::Rejected(razones) = bmo_verify::verify(&bef) {
                eprintln!("error: el BEF no pasa el gate de verificacion:");
                for r in &razones { eprintln!("  - {r}"); }
                std::process::exit(1);
            }
            match fs::write(&salida, &bef) {
        Ok(()) => println!("{} — {} bytes", salida.display(), bef.len()),
        Err(e) => { eprintln!("no se pudo escribir {}: {e}", salida.display()); std::process::exit(1); }
    }
}
