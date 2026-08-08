use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = &args[0];
    let mut base_paths: Vec<PathBuf> = Vec::new();
    let mut asm_paths: Vec<PathBuf> = Vec::new();
    let mut standard = bmo_c_front::CStandard::DefaultC;
    let mut file_path = None;
    let mut out_override: Option<PathBuf> = None;
    let mut solo_preprocesar = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--base" | "-b" => {
                i += 1;
                if i < args.len() {
                    base_paths.push(PathBuf::from(&args[i]));
                } else {
                    eprintln!("error: --base requires a path");
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
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    out_override = Some(PathBuf::from(&args[i]));
                } else {
                    eprintln!("error: -o requires a path");
                    process::exit(2);
                }
            }
            "--std" => {
                i += 1;
                if i < args.len() {
                    match bmo_c_front::CStandard::from_name(&args[i]) {
                        Some(s) => standard = s,
                        None => {
                            eprintln!("error: unknown standard '{}'. Use c89/c99/c11/c17/c23", args[i]);
                            process::exit(2);
                        }
                    }
                } else {
                    eprintln!("error: --std requires a standard name (c89/c99/c11/c17/c23)");
                    process::exit(2);
                }
            }
            // `-E`, el mismo nombre que en cualquier compilador de C: escribe
            // el texto YA preprocesado y no compila nada. Es lo que hace
            // legible un `error:7354:` -- ver `preprocess_only`.
            "-E" | "--preprocess" => {
                solo_preprocesar = true;
            }
            _ => {
                file_path = Some(&args[i]);
            }
        }
        i += 1;
    }

    let Some(path) = file_path else {
        eprintln!("usage: {program} [--std c99] [--base <path>] [--asm-path <path>] <source.c>");
        process::exit(2);
    };

    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("error: cannot read {path}: {err}");
            process::exit(1);
        }
    };

    if solo_preprocesar {
        match bmo_c_front::preprocess_only(&source, Path::new(path), standard) {
            Ok(texto) => {
                println!("{texto}");
                return;
            }
            Err(err) => {
                eprintln!("error:{}: {}", err.line, err.message);
                process::exit(1);
            }
        }
    }

    let result = match (base_paths.is_empty(), asm_paths.is_empty()) {
        (true, true) => {
            let file = Path::new(path);
            bmo_c_front::compile_with_preprocessor(&source, file, standard)
        }
        _ => bmo_c_front::compile_source_to_bef_with_all(&source, base_paths, asm_paths),
    };

    match result {
        Ok(bef_bytes) => {
            // Sin -o la salida es <fuente>.bef. El BEF y el BEX son el
            // MISMO formato (magic BEF1); `.bex` es la extension de uno
            // ejecutable, que es lo que el kernel embebe.
            let out_path = out_override
                .unwrap_or_else(|| Path::new(path).with_extension("bef"));

            // -- * EL GATE, ANTES DE ESCRIBIR --------------------------
            //
            // `bmo-verify` es lo que la filosofia llama **el UNICO checkpoint
            // comun**: lo que reemplaza al rol de seguridad de un IR central,
            // pero como CONTRATO y no como embudo -- cada lenguaje emite su BEF
            // por su cuenta y el verificador lo revisa por separado.
            //
            // Y hasta hoy **no lo llamaba nadie**. El crate existia, delegaba
            // en el validador real de `bmo_abi::bef::validator` (15 tests), y
            // ningun frontend lo consultaba: el gate estaba escrito y abierto.
            //
            // Va ANTES del `write` a proposito. Verificar despues dejaria un
            // fichero malo en el disco con un mensaje de error al lado, y el
            // que lo encuentre manana vera el `.bex` y no el mensaje. Un gate
            // que avisa cuando el dano ya esta hecho es un informe, no un gate.
            if let bmo_verify::Verdict::Rejected(razones) = bmo_verify::verify(&bef_bytes) {
                eprintln!("error: el BEF no pasa el gate de verificacion:");
                for r in &razones {
                    eprintln!("  - {r}");
                }
                eprintln!("  (no se ha escrito {})", out_path.display());
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
