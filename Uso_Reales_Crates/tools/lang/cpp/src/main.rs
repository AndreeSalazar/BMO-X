//! bmo-cpp-front — C++ to BEF compiler.

use std::env;
use std::fs;
use bmo_cpp_front::compile_to_ir;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: bmo-cpp-front <file.cpp>");
        std::process::exit(1);
    }

    let path = &args[1];
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("Error reading {}: {}", path, e); std::process::exit(1); }
    };

    match compile_to_ir(&source) {
        Ok(ir) => {
            println!("OK: compiled to IrModule");
            println!("  functions: {}", ir.function_count);
            println!("  types:     {}", ir.type_count);
            println!("  strings:   {}", ir.string_count);
            println!("  globals:   {}", ir.global_count);
        }
        Err(e) => {
            eprintln!("Error line {}: {}", e.line, e.message);
            std::process::exit(1);
        }
    }
}
