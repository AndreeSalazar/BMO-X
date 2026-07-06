use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    let mut args: Vec<String> = env::args().collect();
    let program = args.remove(0);
    let mut base_paths: Vec<PathBuf> = Vec::new();
    let mut file_path = None;

    let mut i = 0;
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
            _ => {
                file_path = Some(&args[i]);
            }
        }
        i += 1;
    }

    let Some(path) = file_path else {
        eprintln!("usage: {program} [--base <path>] <source.c>");
        process::exit(2);
    };

    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("error: cannot read {path}: {err}");
            process::exit(1);
        }
    };

    let result = if base_paths.is_empty() {
        fastos_c_front::compile_source_to_bef(&source)
    } else {
        fastos_c_front::compile_source_to_bef_with_modules(&source, base_paths)
    };

    match result {
        Ok(bef_bytes) => {
            let out_path = Path::new(path).with_extension("bef");
            match fs::write(&out_path, &bef_bytes) {
                Ok(_) => {
                    println!("ok: wrote {} bytes → {}", bef_bytes.len(), out_path.display());
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
