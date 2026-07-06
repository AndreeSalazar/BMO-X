use std::env;
use std::fs;
use std::path::Path;
use std::process;

fn main() {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "fastos-c-front".to_string());
    let Some(path) = args.next() else {
        eprintln!("usage: {program} <source.c>");
        process::exit(2);
    };

    let source = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("error: cannot read {path}: {err}");
            process::exit(1);
        }
    };

    match fastos_c_front::compile_source_to_bef(&source) {
        Ok(bef_bytes) => {
            let out_path = Path::new(&path).with_extension("bef");
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
