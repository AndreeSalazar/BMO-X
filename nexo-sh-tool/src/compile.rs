//! Compilación de shaders via Naga → SPIR-V

use naga::ShaderStage;

pub fn to_spirv(source: &str, ext: &str, stage: ShaderStage) -> Result<Vec<u8>, String> {
    let module = parse_source(source, ext, stage)?;

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    let module_info = validator
        .validate(&module)
        .map_err(|e| format!("validación falló: {}", e))?;

    let mut spirv = Vec::new();
    let options = naga::back::spv::Options {
        lang_version: (1, 6),
        ..Default::default()
    };
    let mut writer = naga::back::spv::Writer::new(&options)
        .map_err(|e| format!("error creando writer SPIR-V: {}", e))?;
    writer
        .write(
            &module,
            &module_info,
            None,
            &None,
            &mut spirv,
        )
        .map_err(|e| format!("error escribiendo SPIR-V: {:?}", e))?;

    let mut bytes = Vec::with_capacity(spirv.len() * 4);
    for word in &spirv {
        bytes.extend_from_slice(&word.to_le_bytes());
    }

    Ok(bytes)
}

fn parse_source(source: &str, ext: &str, stage: ShaderStage) -> Result<naga::Module, String> {
    match ext {
        "wgsl" => naga::front::wgsl::parse_str(source)
            .map_err(|e| format!("WGSL parse error: {}", e)),
        "glsl" | "vert" | "frag" | "comp" => {
            let mut opts = naga::front::glsl::Options {
                stage: match ext {
                    "vert" => ShaderStage::Vertex,
                    "frag" => ShaderStage::Fragment,
                    "comp" => ShaderStage::Compute,
                    _ => stage,
                },
                defines: Default::default(),
            };
            let mut parser = naga::front::glsl::Frontend::default();
            parser
                .parse(&opts, source)
                .map_err(|errors| {
                    let msgs: Vec<String> = errors.errors.iter().map(|e| format!("{:?}", e)).collect();
                    format!("GLSL parse errors: {}", msgs.join("; "))
                })
        }
        "hlsl" => {
            Err("HLSL parsing requires DXC. Use naga-cli to convert HLSL → SPIR-V first, then pass .spv to nexo-sh".to_string())
        }
        _ => Err(format!("formato no soportado: {}", ext)),
    }
}
