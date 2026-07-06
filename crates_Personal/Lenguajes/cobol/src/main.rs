use std::env;
use std::fs;
use std::process;

fn main() {
    let mut args = env::args();
    let program = args.next().unwrap_or_else(|| "fastos-cobol-front".to_string());
    let Some(path) = args.next() else {
        eprintln!("usage: {program} <source.cob>");
        process::exit(2);
    };

    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("error: cannot read {path}: {err}");
            process::exit(1);
        }
    };

    match fastos_cobol_front::compile_source_to_bmo_ir(&source) {
        Ok(ir) => print!("{ir}"),
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
