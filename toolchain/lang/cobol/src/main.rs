use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = &args[0];
    let mut asm_paths: Vec<PathBuf> = Vec::new();
    let mut file_path = None;
    let mut out_override: Option<PathBuf> = None;
    let mut solo_copybook = false;
    let mut ver_datos: Option<PathBuf> = None;
    let mut ver_registro: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    out_override = Some(PathBuf::from(&args[i]));
                } else {
                    eprintln!("error: -o requires a path");
                    process::exit(2);
                }
            }
            // * El COPYBOOK: el byte exacto de cada campo, sacado de la MISMA
            // tabla que emite el READ y el WRITE. No compila nada -- ensena el
            // formato del fichero y se va.
            "--copybook" => {
                solo_copybook = true;
            }
            // * El VISOR: ensena un fichero de registros binarios decodificado
            // con el copybook de este programa. Desde que un COMP-3 sale al
            // disco, ese fichero no se puede mirar con un `cat`.
            "--ver" => {
                i += 1;
                if i < args.len() {
                    ver_datos = Some(PathBuf::from(&args[i]));
                } else {
                    eprintln!("error: --ver necesita el fichero de datos");
                    process::exit(2);
                }
            }
            "--registro" => {
                i += 1;
                if i < args.len() {
                    ver_registro = Some(args[i].clone());
                } else {
                    eprintln!("error: --registro necesita un nombre");
                    process::exit(2);
                }
            }
            "--asm-path" | "-a" => {
                i += 1;
                if i < args.len() {
                    asm_paths.push(PathBuf::from(&args[i]));
                } else {
                    eprintln!("error: --asm-path requires a path");
                    process::exit(2);
                }
            }
            _ => {
                file_path = Some(&args[i]);
            }
        }
        i += 1;
    }

    let Some(path) = file_path else {
        eprintln!(
            "usage: {program} [-o <salida.bex>] [--copybook] [--asm-path <path>] <source.cob>"
        );
        process::exit(2);
    };

    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("error: cannot read {path}: {err}");
            process::exit(1);
        }
    };

    // * El copybook sale del PARSER, no del binario: ensena el formato aunque
    // el programa todavia no compile entero. Quien tiene que acordar un fichero
    // con otro equipo no puede esperar a que el batch este terminado.
    if let Some(datos_path) = ver_datos {
        let datos = match fs::read(&datos_path) {
            Ok(d) => d,
            Err(err) => {
                eprintln!("error: no se puede leer {}: {err}", datos_path.display());
                process::exit(1);
            }
        };
        match bmo_cobol_front::ver_registros(&source, &datos, ver_registro.as_deref(), 50) {
            Ok(texto) => {
                println!("* {}", datos_path.display());
                print!("{texto}");
                return;
            }
            Err(err) => {
                eprintln!("error: {}", err.message);
                process::exit(1);
            }
        }
    }

    if solo_copybook {
        match bmo_cobol_front::copybook_de(&source) {
            Ok(texto) => {
                print!("{texto}");
                return;
            }
            Err(err) => {
                if err.line == 0 {
                    eprintln!("error: {}", err.message);
                } else {
                    eprintln!("error:{}: {}", err.line, err.message);
                }
                process::exit(1);
            }
        }
    }

    let result = if asm_paths.is_empty() {
        bmo_cobol_front::compile_source_to_bex(&source)
    } else {
        bmo_cobol_front::compile_source_to_bex_with_asm(&source, asm_paths)
    };

    match result {
        Ok(bef_bytes) => {
            let out_path = out_override.unwrap_or_else(|| {
                Path::new(path).with_extension(bmo_abi::bex::BEX_EXTENSION)
            });
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
            if let bmo_verify::Verdict::Rejected(razones) = bmo_verify::verify(&bef_bytes) {
                eprintln!("error: el BEF no pasa el gate de verificacion:");
                for r in &razones { eprintln!("  - {r}"); }
                std::process::exit(1);
            }
            match fs::write(&out_path, &bef_bytes) {
                Ok(_) => {
                    println!("ok: wrote {} bytes -> {}", bef_bytes.len(), out_path.display());
                }
                Err(err) => {
                    eprintln!("error: cannot write {}: {}", out_path.display(), err);
                    process::exit(1);
                }
            }
        }
        Err(err) => {
            if err.line == 0 {
                eprintln!("error: {}", err.message);
            } else {
                eprintln!("error:{}: {}", err.line, err.message);
            }
            process::exit(1);
        }
    }
}
