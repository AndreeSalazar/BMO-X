//! Estandares de C soportados (C89..C23) -- el modulo de las VERSIONES.
//! Cada version es un TOML en forge/sem-asm/tables/standards/C/ (titulo propio),
//! y todas se unen aqui en StandardFeatures: un solo mecanismo de gating.

use std::path::{Path, PathBuf};

/// C standard version to compile against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CStandard {
    C89,
    C99,
    C11,
    C17,
    C23,
    DefaultC, // uses C99 as baseline
}

impl CStandard {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "c89" | "c90" => Some(Self::C89),
            "c99" => Some(Self::C99),
            "c11" => Some(Self::C11),
            "c17" | "c18" => Some(Self::C17),
            "c23" => Some(Self::C23),
            "default" => Some(Self::DefaultC),
            _ => None,
        }
    }

    pub fn toml_name(&self) -> &str {
        match self {
            Self::C89 => "c89.toml",
            Self::C99 | Self::DefaultC => "c99.toml",
            Self::C11 => "c11.toml",
            Self::C17 => "c17.toml",
            Self::C23 => "c23.toml",
        }
    }
}

/// Standard feature set loaded from a cXX.toml manifest.
#[derive(Debug, Clone)]
pub struct StandardFeatures {
    pub line_comments: bool,
    pub long_long: bool,
    pub inline: bool,
    pub restrict: bool,
    pub variadic_macros: bool,
    pub compound_literals: bool,
    pub designated_initializers: bool,
    pub mixed_declarations: bool,
    pub implicit_int: bool,
    pub implicit_function_decl: bool,
    pub return_without_value: bool,
}

impl Default for StandardFeatures {
    fn default() -> Self {
        Self {
            line_comments: true,
            long_long: true,
            inline: true,
            restrict: true,
            variadic_macros: true,
            compound_literals: true,
            designated_initializers: true,
            mixed_declarations: true,
            implicit_int: false,
            implicit_function_decl: false,
            return_without_value: false,
        }
    }
}

impl StandardFeatures {
    /// Load features from a forge/sem-asm/tables/standards/C/cXX.toml file.
    pub fn load_from_toml(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        let mut feats = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') { continue; }
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim().trim_matches('"');
                let enabled = val == "true" || val == "1";
                match key {
                    "line_comments" => feats.line_comments = enabled,
                    "long_long" => feats.long_long = enabled,
                    "inline" => feats.inline = enabled,
                    "restrict" => feats.restrict = enabled,
                    "variadic_macros" => feats.variadic_macros = enabled,
                    "compound_literals" => feats.compound_literals = enabled,
                    "designated_initializers" => feats.designated_initializers = enabled,
                    "mixed_declarations_and_code" => feats.mixed_declarations = enabled,
                    "implicit_int" => feats.implicit_int = enabled,
                    "implicit_function_decl" => feats.implicit_function_decl = enabled,
                    "return_without_value_allowed" => feats.return_without_value = enabled,
                    _ => {}
                }
            }
        }
        Some(feats)
    }

    /// Carga desde las tablas sem-asm (forge/sem-asm/tables/standards/C).
    /// NOTA: las rutas viejas apuntaban a "Semantic_ASM/" (directorio que ya
    /// no existe tras la reorganización) — el gating de estándares caía en
    /// silencio al default. Ahora apunta a la única fuente de verdad.
    pub fn load_standard(std: CStandard) -> Self {
        // relativo al cwd (raíz del repo o subdirectorios comunes)
        let candidates = &[
            "toolchain/forge/sem-asm/tables/standards/C",
            "forge/sem-asm/tables/standards/C",
            "../forge/sem-asm/tables/standards/C",
            "../../forge/sem-asm/tables/standards/C",
            "../toolchain/forge/sem-asm/tables/standards/C",
        ];
        for c in candidates {
            let p = PathBuf::from(c).join(std.toml_name());
            if p.exists() {
                if let Some(feats) = Self::load_from_toml(&p) {
                    return feats;
                }
            }
        }
        // relativo a la crate (toolchain/lang/c → ../../forge/sem-asm/tables)
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let p = PathBuf::from(&manifest_dir)
                .join("../../forge/sem-asm/tables/standards/C")
                .join(std.toml_name());
            if p.exists() {
                if let Some(feats) = Self::load_from_toml(&p) {
                    return feats;
                }
            }
        }
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c89_gating_loads_from_real_tables() {
        // Prueba que load_standard ENCUENTRA las tablas sem-asm de verdad.
        // C89: line_comments=false difiere del default (true) → si esto pasa,
        // el TOML se leyó (con la ruta muerta de antes, caía al default en silencio).
        let f = StandardFeatures::load_standard(CStandard::C89);
        assert!(!f.line_comments, "C89 no tiene // (si esto falla, el TOML no cargó)");
        assert!(!f.long_long, "C89 no tiene long long");
        assert!(f.implicit_int, "C89 permite int implícito");
        assert!(f.implicit_function_decl, "C89 permite declaración implícita");
    }

    #[test]
    fn c99_gating_loads_from_real_tables() {
        let f = StandardFeatures::load_standard(CStandard::C99);
        assert!(f.line_comments, "C99 tiene //");
        assert!(f.long_long);
        assert!(!f.implicit_int, "C99 eliminó el int implícito");
    }

    #[test]
    fn all_standards_have_a_table() {
        for std in [CStandard::C89, CStandard::C99, CStandard::C11, CStandard::C17, CStandard::C23] {
            let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../forge/sem-asm/tables/standards/C");
            let p = dir.join(std.toml_name());
            assert!(p.exists(), "falta la tabla {} en forge/sem-asm", std.toml_name());
        }
    }
}

