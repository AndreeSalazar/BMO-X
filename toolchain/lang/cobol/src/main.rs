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
        eprintln!("usage: {program} [-o <salida.bex>] [--asm-path <path>] <source.cob>");
        process::exit(2);
    };

    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("error: cannot read {path}: {err}");
            process::exit(1);
        }
    };

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
            match fs::write(&out_path, &bef_bytes) {
                Ok(_) => {
                    println!("ok: wrote {} bytes â†’ {}", bef_bytes.len(), out_path.display());
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
