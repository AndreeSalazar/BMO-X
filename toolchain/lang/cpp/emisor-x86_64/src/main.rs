//! `bmo-cpp-front` -- compila C++ a BEF.
//!
//! Antes imprimia los contadores de un `IrModule` que nadie consumia: decia
//! "OK: compiled" sin haber producido un solo byte ejecutable. Ahora escribe
//! el `.bex` o falla diciendo por que.
//!
//! ** 2026-09-17: the command line is the same as Ada's and C's --
//! `<fuente> [-o salida]` -- because `build/ejemplos.ps1::Compilar-Ejemplos`
//! calls every frontend that way. The old form took the SECOND argument as the
//! output path, so `-o destino.bex` wrote a file literally named `-o`, and the
//! destination the build then looked for did not exist. That is part of why C++
//! never reached the disk.

use std::path::PathBuf;
use std::process;

use bmo_cpp_x86_64::{compile_source_to_bef, compile_source_to_object};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut fuente: Option<String> = None;
    let mut salida: Option<PathBuf> = None;
    // `-c`: una UNIDAD (`.bo`) en vez de un programa, para `bmo-enlazar`.
    // Mismo nombre de bandera que BMO C y que cualquier compilador de C.
    let mut objeto = false;

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
            "-c" | "--objeto" => objeto = true,
            otro if fuente.is_none() => fuente = Some(otro.to_string()),
            otro => {
                eprintln!("error: argumento de mas: {otro}");
                process::exit(2);
            }
        }
        i += 1;
    }

    let Some(ruta) = fuente else {
        eprintln!("uso: bmo-cpp-front <fichero.cpp> [-o salida.bex] [-c]");
        process::exit(2);
    };

    let texto = match std::fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: no puedo leer {ruta}: {e}");
            process::exit(1);
        }
    };

    let bef = match if objeto { compile_source_to_object(&texto) } else { compile_source_to_bef(&texto) } {
        Ok(b) => b,
        Err(e) => {
            if e.line > 0 {
                eprintln!("{ruta}: error en la linea {}: {}", e.line, e.message);
            } else {
                eprintln!("{ruta}: error: {}", e.message);
            }
            process::exit(1);
        }
    };

    let destino = salida.unwrap_or_else(|| {
        let mut p = PathBuf::from(&ruta);
        p.set_extension(if objeto { "bo" } else { "bex" });
        p
    });

    // -- * EL GATE, ANTES DE ESCRIBIR --------------------------
    //
    // `bmo-verify` es el "unico checkpoint comun" de la filosofia: el papel de
    // seguridad que tendria un IR central, pero como CONTRATO -- cada lenguaje
    // emite su BEF por su cuenta y el verificador lo revisa por separado.
    //
    // Va ANTES del `write` a proposito: verificar despues dejaria un fichero
    // malo en el disco con un mensaje al lado, y quien lo encuentre manana vera
    // el `.bex`, no el mensaje.
    //
    // ** Y a un OBJETO se le pide lo del objeto, no lo del programa: un `.bo`
    // no tiene punto de entrada porque `main` vive en otra unidad. Pasarle el
    // gate del programa lo rechazaba por no ser lo que no pretende ser.
    let veredicto = if objeto {
        bmo_verify::verify_object(&bef)
    } else {
        bmo_verify::verify(&bef)
    };
    if let bmo_verify::Verdict::Rejected(razones) = veredicto {
        eprintln!("error: el BEF no pasa el gate de verificacion:");
        for r in &razones {
            eprintln!("  - {r}");
        }
        process::exit(1);
    }
    if let Some(padre) = destino.parent() {
        if !padre.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(padre) {
                eprintln!("error: no puedo crear {}: {e}", padre.display());
                process::exit(1);
            }
        }
    }
    match std::fs::write(&destino, &bef) {
        Ok(()) => println!("ok: wrote {} bytes -> {}", bef.len(), destino.display()),
        Err(e) => {
            eprintln!("error: no puedo escribir {}: {e}", destino.display());
            process::exit(1);
        }
    }
}
