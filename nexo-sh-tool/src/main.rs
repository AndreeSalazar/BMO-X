//! nexo-sh — Naga en Ring 3, compilador HLSL/GLSL/WGSL → BSF
//!
//! Usa: nexo-sh <input.hlsl|glsl|wgsl> -o <output.bsf> [--stage vertex|fragment|compute]
//!
//! El BSF resultante está firmado con BLAKE3 y listo para BareX driver en Ring 0.

use std::{env, fs, process};

mod bsf;
mod compile;
mod hash;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("uso: nexo-sh <input> -o <output.bsf> [--stage vertex|fragment|compute]");
        eprintln!("  formatos: .hlsl .glsl .wgsl .vert .frag .comp");
        process::exit(1);
    }

    let mut input_path = String::new();
    let mut output_path = String::new();
    let mut stage = "fragment";

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                output_path = args.get(i).cloned().unwrap_or_default();
            }
            "--stage" => {
                i += 1;
                stage = args.get(i).map(|s| s.as_str()).unwrap_or("fragment");
            }
            _ if input_path.is_empty() => {
                input_path = args[i].clone();
            }
            _ => {
                eprintln!("argumento desconocido: {}", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    if input_path.is_empty() || output_path.is_empty() {
        eprintln!("falta input o -o output");
        process::exit(1);
    }

    let source = fs::read_to_string(&input_path).unwrap_or_else(|e| {
        eprintln!("no se puede leer {}: {}", input_path, e);
        process::exit(1);
    });

    let ext = input_path.rsplit('.').next().unwrap_or("");
    let shader_stage = match stage {
        "vertex" | "vs" => naga::ShaderStage::Vertex,
        "fragment" | "ps" => naga::ShaderStage::Fragment,
        "compute" | "cs" => naga::ShaderStage::Compute,
        _ => {
            eprintln!("stage desconocido: {}", stage);
            process::exit(1);
        }
    };

    eprintln!("[nexo-sh] compilando {} → BSF (stage={:?})", input_path, shader_stage);

    let spirv_bytes = match compile::to_spirv(&source, ext, shader_stage) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error de compilación: {}", e);
            process::exit(1);
        }
    };

    let blake3_hash = hash::blake3(&spirv_bytes);

    let arch = match std::env::consts::ARCH {
        "x86_64" => bsf::BsfArch::X86_64,
        "aarch64" => bsf::BsfArch::Aarch64,
        "riscv64" => bsf::BsfArch::Riscv64,
        _ => bsf::BsfArch::X86_64,
    };

    let bsf_bytes = bsf::BsfFile {
        arch,
        stage: shader_stage,
        entry: "main",
        blake3: blake3_hash,
        spirv: &spirv_bytes,
    }
    .to_bytes();

    fs::write(&output_path, &bsf_bytes).unwrap_or_else(|e| {
        eprintln!("no se puede escribir {}: {}", output_path, e);
        process::exit(1);
    });

    eprintln!("[nexo-sh] BSF generado: {} ({} bytes)", output_path, bsf_bytes.len());
    // Print BLAKE3 hash as hex
    for b in &bsf_bytes[8..40] {
        eprint!("{:02x}", b);
    }
    eprintln!();
}
