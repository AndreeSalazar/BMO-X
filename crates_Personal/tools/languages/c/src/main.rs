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

    let result = match (base_paths.is_empty(), asm_paths.is_empty()) {
        (true, true) => {
            let file = Path::new(path);
            bmo_c_front::compile_with_preprocessor(&source, file, standard)
        }
        _ => bmo_c_front::compile_source_to_bef_with_all(&source, base_paths, asm_paths),
    };

    match result {
        Ok(bef_bytes) => {
            let out_path = Path::new(path).with_extension("bef");
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
