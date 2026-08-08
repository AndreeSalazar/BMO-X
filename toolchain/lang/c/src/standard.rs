//! Estandares de C soportados (C89..C23) -- el modulo de las VERSIONES.
//! Cada version es un TOML en forge/sem-asm/tables/standards/C/ (titulo propio),
//! y todas se unen aqui en StandardFeatures: un solo mecanismo de gating.
//!
//! **Quien lee el TOML es `bmo-mods`**, no este modulo. Aqui habia un lector
//! hecho a mano que partia lineas por `=` e IGNORABA las secciones: metia
//! `[features]`, `[type_rules]` y `[predefined_macros]` en el mismo saco.
//! Funcionaba porque ninguna clave se repetia entre secciones -- por suerte,
//! no por diseno. Y llevaba copiada la lista de rutas candidatas que un dia
//! se quedo muerta y dejo el gating cayendo al default en silencio.
//!
//! `StandardFeatures` sigue siendo el struct de las once caracteristicas que
//! el parser consulta a diario, porque preguntarlas por cadena en el camino
//! caliente no mejora nada. Lo que cambia es que **ya no es la unica forma de
//! preguntar**: `StandardFeatures::table()` da la tabla entera, y por ahi se
//! lee cualquier clave que un mod haya anadido sin que este Rust la conozca.

use bmo_mods::{Roots, Standard};
use std::path::Path;

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

    /// El nombre sin `.toml`, que es como lo pide `bmo_mods::Standard::load`.
    pub fn table_name(&self) -> &str {
        self.toml_name().trim_end_matches(".toml")
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
    /// La tabla entera, si se cargo. Es la puerta a lo que este struct no
    /// sabe: un mod que anade `mi_extension = true` se lee por aqui sin
    /// tocar Rust. Ver `bmo_mods`.
    tabla: Option<Standard>,
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
            tabla: None,
        }
    }
}

impl StandardFeatures {
    /// Load features from a forge/sem-asm/tables/standards/C/cXX.toml file.
    ///
    /// Se conserva porque tiene llamadores y porque cargar un fichero suelto
    /// --uno que un tercero acaba de escribir-- es legitimo. Por dentro ya no
    /// parte lineas: delega en el parser de verdad.
    pub fn load_from_toml(path: &Path) -> Option<Self> {
        let dir = path.parent()?;
        let name = path.file_stem()?.to_str()?;
        // Un fichero suelto se carga tratando su propio directorio como la
        // raiz: `<dir>/standards/C/<x>.toml` no existe, asi que se apunta a
        // la abuela para que la ruta relativa cuadre.
        let raiz = dir.parent().and_then(|p| p.parent())?;
        let lang = dir.file_name()?.to_str()?;
        let roots = Roots::from_paths(vec![raiz.to_path_buf()]);
        Standard::load(&roots, lang, name).ok().map(Self::from_table)
    }

    /// Carga desde las tablas sem-asm, o desde donde diga `$BMO_MODS`.
    ///
    /// NOTA HISTORICA: las rutas viejas apuntaban a "Semantic_ASM/"
    /// (directorio que ya no existe tras la reorganizacion) y el gating caia
    /// al default **en silencio**. Ahora el descubrimiento sube desde el cwd
    /// buscando la raiz del repo, y vive en un solo sitio (`bmo_mods::Roots`)
    /// en vez de copiado aqui y en `module.rs`.
    pub fn load_standard(std: CStandard) -> Self {
        let roots = Roots::find();
        match Standard::load(&roots, "C", std.table_name()) {
            Ok(t) => Self::from_table(t),
            Err(_) => Self::default(),
        }
    }

    /// Traduce la tabla a las once que el parser consulta a diario.
    ///
    /// Cada `unwrap_or` lleva el default del struct, que es C99-ish: un
    /// estandar que no menciona una caracteristica hereda la base, y eso es
    /// justo por lo que C89 tiene que apagarlas **explicitamente** en su TOML.
    fn from_table(t: Standard) -> Self {
        let d = Self::default();
        Self {
            // `[features]` -- lo que el lenguaje TIENE.
            line_comments: t.flag("features", "line_comments").unwrap_or(d.line_comments),
            long_long: t.flag("features", "long_long").unwrap_or(d.long_long),
            inline: t.flag("features", "inline").unwrap_or(d.inline),
            restrict: t.flag("features", "restrict").unwrap_or(d.restrict),
            variadic_macros: t.flag("features", "variadic_macros").unwrap_or(d.variadic_macros),
            compound_literals: t.flag("features", "compound_literals").unwrap_or(d.compound_literals),
            designated_initializers: t
                .flag("features", "designated_initializers")
                .unwrap_or(d.designated_initializers),
            mixed_declarations: t
                .flag("features", "mixed_declarations_and_code")
                .unwrap_or(d.mixed_declarations),
            // `[type_rules]` -- lo que el lenguaje PERMITE. Son otra seccion y
            // ahora se leen de ella: antes daba igual porque el lector era
            // plano, y habria bastado una clave repetida para cruzarlas.
            implicit_int: t.rule("implicit_int").unwrap_or(d.implicit_int),
            implicit_function_decl: t
                .rule("implicit_function_decl")
                .unwrap_or(d.implicit_function_decl),
            return_without_value: t
                .rule("return_without_value_allowed")
                .unwrap_or(d.return_without_value),
            tabla: Some(t),
        }
    }

    /// La tabla de la que salio esto, si salio de alguna.
    ///
    /// Es la puerta de los mods: `feats.table().map(|t| t.on("mi_extension"))`
    /// lee una clave que ningun Rust de este repo conoce.
    pub fn table(&self) -> Option<&Standard> {
        self.tabla.as_ref()
    }

    /// Esta encendida esta caracteristica? Pregunta por nombre, para lo que
    /// el struct no tiene campo. Sin tabla cargada, `false`.
    pub fn on(&self, feature: &str) -> bool {
        self.tabla.as_ref().is_some_and(|t| t.on(feature))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c89_gating_loads_from_real_tables() {
        // Prueba que load_standard ENCUENTRA las tablas sem-asm de verdad.
        // C89: line_comments=false difiere del default (true) -> si esto pasa,
        // el TOML se leyo (con la ruta muerta de antes, caia al default en silencio).
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

