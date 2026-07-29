//! **El contrato de los mods de BMO** — formato, nunca cerebro.
//!
//! Librería que cada frontend ELIGE enlazar, como el resto de `forge/`. No es
//! un embudo: quien no la quiera sigue leyendo sus ficheros a mano.
//!
//! ## Qué problema resuelve
//!
//! Un comité decide qué entra en un lenguaje y cuándo. Eso da estabilidad y
//! cuesta años por cada cambio. La alternativa que ya practica este repo —
//! *"añadir una instrucción = 1 entrada TOML, CERO Rust"*— no necesita comité:
//! el que quiere una extensión la declara y la usa.
//!
//! Lo que faltaba no era la idea, era **un solo formato**. Había tres:
//!
//! - `lang/c/src/module.rs` leía `BMO.toml` partiendo líneas por `=`.
//! - `lang/c/src/standard.rs` leía `standards/C/*.toml` igual, y **se comía
//!   las secciones**: `[features]` y `[type_rules]` caían en el mismo saco.
//!   Funcionaba por suerte, porque ninguna clave se repetía todavía.
//! - `forge/sem-asm` sí usaba un parser de verdad, para otras tablas.
//!
//! Y los dos primeros llevaban copiada la misma lista de cinco rutas
//! candidatas para encontrar `tables/`. Cuando esa lista se quedó vieja, el
//! gating de estándares **cayó al default en silencio** durante meses — está
//! escrito en el comentario de `standard.rs`. Un formato con tres lectores
//! tiene tres formas de mentir.
//!
//! ## La frontera honesta
//!
//! Esto quita el Rust de **DECLARAR** una extensión, no de implementarla.
//! Añadir `mi_extension = true` a un TOML es gratis; que el compilador haga
//! algo distinto sigue siendo código. Es la misma frontera que la fábrica de
//! COBOL: lo tabular se genera, la semántica de cada verbo se escribe.
//!
//! Prometer más que eso sería vender compatibilidad que no existe.
//!
//! ## Dónde se buscan los mods
//!
//! En este orden, y el primero que tenga el fichero gana:
//!
//! 1. `$BMO_MODS` — una o varias rutas separadas por `;`. **Es la puerta de
//!    los terceros**: se dejan ahí y no se toca el repo. Va primero a
//!    propósito, para poder tapar una tabla del sistema sin editarla.
//! 2. `mods/` en la raíz del repo, si existe.
//! 3. `toolchain/forge/sem-asm/tables/` — las tablas del sistema.
//!
//! ## Lo que esto NO hace, y hay que decirlo
//!
//! Un mod de tabla es **datos**: no ejecuta nada, así que no puede robar
//! nada. El día que un mod sea un `.bex` con código, esto no basta: haría
//! falta el gate de firma y `bmo-verify` — que hoy está escrito y **no tiene
//! un solo usuario**. Ese es el orden correcto y por eso se empieza por aquí.

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ModError {
    /// No se encontró ninguna raíz con tablas. Casi siempre es el cwd.
    SinRaices,
    /// El fichero no está en ninguna de las raíces.
    NoEsta { que: String, buscado_en: Vec<PathBuf> },
    /// El fichero está pero no se puede leer.
    NoSeLee(PathBuf),
    /// El fichero está y no es TOML válido. Se dice CUÁL y POR QUÉ: un mod
    /// ajeno mal escrito tiene que señalar a su autor, no al sistema.
    NoEsToml { fichero: PathBuf, motivo: String },
    /// Una cadena de herencia que se muerde la cola. Se para y se enseña
    /// entera: `a → b → a` colgaría el compilador, y un compilador colgado no
    /// dice de quién es la culpa.
    Ciclo { cadena: Vec<String> },
}

impl std::fmt::Display for ModError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SinRaices => write!(f, "no encuentro las tablas de BMO (¿cwd raro? prueba $BMO_MODS)"),
            Self::NoEsta { que, buscado_en } => {
                write!(f, "no existe '{que}'; buscado en:")?;
                for p in buscado_en {
                    write!(f, "\n  {}", p.display())?;
                }
                Ok(())
            }
            Self::NoSeLee(p) => write!(f, "no puedo leer {}", p.display()),
            Self::NoEsToml { fichero, motivo } => {
                write!(f, "{} no es TOML valido: {motivo}", fichero.display())
            }
            Self::Ciclo { cadena } => {
                write!(f, "herencia circular: {}", cadena.join(" -> "))
            }
        }
    }
}

impl std::error::Error for ModError {}

// ── Dónde vive todo ─────────────────────────────────────────────────────

/// Las raíces donde se buscan tablas y módulos, en orden de prioridad.
///
/// Existe para que la lista de rutas candidatas esté **en un sitio**. Estaba
/// copiada en dos, y cuando el repo se reorganizó una de las copias apuntó
/// durante meses a un directorio muerto sin que nadie se enterara: el
/// descubrimiento fallaba devolviendo "no hay", que se parece demasiado a
/// "no hace falta".
#[derive(Debug, Clone)]
pub struct Roots {
    paths: Vec<PathBuf>,
}

impl Roots {
    /// Descubre las raíces. Nunca falla: puede devolver una lista vacía, y
    /// entonces cada `load` dirá exactamente dónde miró.
    pub fn find() -> Self {
        let mut paths = Vec::new();

        // 1. Los mods de terceros, primero. Poder tapar una tabla del sistema
        //    sin editarla es la diferencia entre extender y bifurcar.
        if let Ok(v) = std::env::var("BMO_MODS") {
            for trozo in v.split(';') {
                let p = PathBuf::from(trozo.trim());
                if !trozo.trim().is_empty() && p.is_dir() {
                    paths.push(p);
                }
            }
        }

        // 2 y 3. La raíz del repo, buscada hacia arriba desde el cwd. Subir
        //    en vez de listar rutas relativas a ojo: así funciona igual desde
        //    la raíz, desde `toolchain/lang/c` o desde donde sea.
        if let Some(repo) = Self::repo_root() {
            let mods = repo.join("mods");
            if mods.is_dir() {
                paths.push(mods);
            }
            let tablas = repo.join("toolchain/forge/sem-asm/tables");
            if tablas.is_dir() {
                paths.push(tablas);
            }
        }

        Self { paths }
    }

    /// Construye unas raíces explícitas. Para los tests y para quien quiera
    /// mandar él.
    pub fn from_paths(paths: Vec<PathBuf>) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// El primer sitio donde existe `rel`.
    pub fn locate(&self, rel: &str) -> Option<PathBuf> {
        self.paths.iter().map(|p| p.join(rel)).find(|p| p.exists())
    }

    fn locate_or_err(&self, rel: &str) -> Result<PathBuf, ModError> {
        if self.paths.is_empty() {
            return Err(ModError::SinRaices);
        }
        self.locate(rel).ok_or_else(|| ModError::NoEsta {
            que: rel.to_string(),
            buscado_en: self.paths.iter().map(|p| p.join(rel)).collect(),
        })
    }

    /// Sube desde el cwd hasta encontrar la marca del repo. `Cargo.toml` solo
    /// no vale —hay uno en cada crate—; la marca es el par raíz.
    fn repo_root() -> Option<PathBuf> {
        let mut dir = std::env::current_dir().ok()?;
        loop {
            if dir.join("toolchain/forge/sem-asm/tables").is_dir() {
                return Some(dir);
            }
            if !dir.pop() {
                return None;
            }
        }
    }
}

fn leer_toml(path: &Path) -> Result<toml::Table, ModError> {
    let texto = std::fs::read_to_string(path).map_err(|_| ModError::NoSeLee(path.to_path_buf()))?;
    texto.parse().map_err(|e| ModError::NoEsToml {
        fichero: path.to_path_buf(),
        motivo: format!("{e}"),
    })
}

// ── Un estándar (o un dialecto de nadie) ────────────────────────────────

/// Una capa de la cadena de herencia: un fichero y de dónde salió.
#[derive(Debug, Clone)]
struct Layer {
    name: String,
    path: PathBuf,
    table: toml::Table,
}

/// Un fichero de `standards/<LENGUAJE>/<nombre>.toml`, **con su herencia
/// resuelta**.
///
/// Se llama "estándar" porque hoy los que hay son de comité (C11, COBOL85,
/// C++17), pero el formato no distingue: un dialecto propio en
/// `standards/C/mio.toml` se carga por la misma puerta y con el mismo peso.
/// **Ésa es la idea entera.**
///
/// ## Las tres capas, y por qué son tres
///
/// El mismo mecanismo da tres posturas distintas, y no hace falta un
/// mecanismo por postura:
///
/// | Quiero… | Se escribe |
/// |---|---|
/// | el estándar de BMO tal cual | nada |
/// | **mi propio estándar** | una tabla SIN `parent` |
/// | **añadir cosas a uno** | una tabla CON `parent` y sólo el delta |
///
/// La tercera es la que evita que esto sea anarquía. Un mod que declara
/// `parent = "c11"` y tres claves **no puede bifurcar el resto**: el día que
/// BMO corrige c11, ese mod se lleva la corrección. Copiar la tabla entera
/// para cambiar tres líneas sí es una bifurcación, y es lo que había que
/// hacer antes de esto.
///
/// ## Las capas no se funden
///
/// Se guardan en orden hijo→padre y cada consulta baja hasta el primero que
/// contesta. Cuesta lo mismo y permite `origin()`: saber **qué fichero** puso
/// un valor. En un sistema donde cualquiera puede tapar una tabla, "¿de dónde
/// ha salido esto?" es la primera pregunta que se hace todo el mundo.
///
/// ## Las características no son un `struct` de Rust
///
/// Antes lo eran, con un `match` de once claves: añadir una exigía tocar tres
/// sitios de Rust —el campo, el `Default` y el `match`—, que es exactamente el
/// trámite de comité del que se quería salir. Aquí se preguntan por nombre y
/// una clave nueva no necesita recompilar nada.
#[derive(Debug, Clone)]
pub struct Standard {
    lang: String,
    layers: Vec<Layer>,
}

/// Tope de la cadena. Existe para que un error de escritura no se lleve por
/// delante la pila; el ciclo lo caza la lista de visitados, esto es el cinturón.
const MAX_HERENCIA: usize = 32;

impl Standard {
    /// Carga `standards/<lang>/<name>.toml` y su cadena de `[based_on] parent`.
    ///
    /// El padre se busca por las MISMAS raíces, así que un mod puede heredar
    /// de una tabla del sistema — y si alguien tapó esa tabla, hereda de la
    /// tapada. Es lo coherente: una raíz que va primero, va primero siempre.
    pub fn load(roots: &Roots, lang: &str, name: &str) -> Result<Self, ModError> {
        let mut layers: Vec<Layer> = Vec::new();
        let mut vistos: Vec<String> = Vec::new();
        let mut actual = name.to_string();

        loop {
            if vistos.iter().any(|v| *v == actual) {
                vistos.push(actual);
                return Err(ModError::Ciclo { cadena: vistos });
            }
            vistos.push(actual.clone());

            let rel = format!("standards/{lang}/{actual}.toml");
            let path = roots.locate_or_err(&rel)?;
            let table = leer_toml(&path)?;

            // El padre se lee ANTES de mover la tabla a su capa.
            let padre = table
                .get("based_on")
                .and_then(|v| v.get("parent"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .filter(|s| !s.trim().is_empty());

            layers.push(Layer { name: actual, path, table });

            match padre {
                Some(p) if layers.len() < MAX_HERENCIA => actual = p,
                Some(_) => return Err(ModError::Ciclo { cadena: vistos }),
                None => break,
            }
        }

        Ok(Self { lang: lang.to_string(), layers })
    }

    pub fn lang(&self) -> &str {
        &self.lang
    }

    pub fn name(&self) -> &str {
        &self.layers[0].name
    }

    /// De qué fichero salió. Un mensaje de error que no dice esto obliga a
    /// adivinar cuál de las raíces ganó.
    pub fn path(&self) -> &Path {
        &self.layers[0].path
    }

    /// La cadena de herencia, del hijo al ancestro: `["c23","c17","c11",…]`.
    pub fn lineage(&self) -> Vec<&str> {
        self.layers.iter().map(|l| l.name.as_str()).collect()
    }

    /// Qué tabla de la cadena puso este valor. `None` si nadie lo pone.
    ///
    /// Es la respuesta a "¿por qué mi flag no hace efecto?" — casi siempre
    /// porque otra capa lo dice antes.
    pub fn origin(&self, section: &str, key: &str) -> Option<&str> {
        self.layers
            .iter()
            .find(|l| l.table.get(section).and_then(|s| s.get(key)).is_some())
            .map(|l| l.name.as_str())
    }

    /// El valor crudo, bajando por la cadena hasta el primero que contesta.
    fn lookup(&self, section: &str, key: &str) -> Option<&toml::Value> {
        self.layers
            .iter()
            .find_map(|l| l.table.get(section).and_then(|s| s.get(key)))
    }

    /// ¿Está encendida esta característica en `[features]`?
    ///
    /// Ausente es `false`, y no es lo mismo que un error: un estándar viejo
    /// simplemente no menciona lo que aún no existía.
    pub fn on(&self, feature: &str) -> bool {
        self.flag("features", feature).unwrap_or(false)
    }

    /// Una regla de `[type_rules]`. Aquí `None` SÍ importa y por eso no se
    /// aplana a `false`: "C89 permite `int` implícito" y "esta tabla no dice
    /// nada del `int` implícito" llevan a compiladores distintos.
    pub fn rule(&self, key: &str) -> Option<bool> {
        self.flag("type_rules", key)
    }

    /// Un booleano de cualquier sección. La sección importa: leer el TOML
    /// plano juntaba `[features]` con `[type_rules]` y con
    /// `[predefined_macros]`, y bastaba una clave repetida entre secciones
    /// para que una pisara a la otra.
    pub fn flag(&self, section: &str, key: &str) -> Option<bool> {
        self.lookup(section, key)?.as_bool()
    }

    /// Un texto de cualquier sección (`[standard].iso_number`, …).
    ///
    /// OJO: `[standard]` también se hereda, así que un mod que no se ponga
    /// `short_name` enseña el de su padre. Es deliberado — un mod que sólo
    /// añade tres claves no tiene por qué reescribir la ficha entera.
    pub fn text(&self, section: &str, key: &str) -> Option<&str> {
        self.lookup(section, key)?.as_str()
    }

    /// Un entero de cualquier sección (`[standard].year`, …).
    pub fn number(&self, section: &str, key: &str) -> Option<i64> {
        self.lookup(section, key)?.as_integer()
    }

    /// Todas las claves de una sección **de toda la cadena**, ordenadas y sin
    /// repetir. Con esto un frontend puede recorrer lo que la tabla trae
    /// **sin conocerlo de antemano** — que es lo que hace falta para que un
    /// mod añada algo que el compilador no tenía previsto.
    pub fn keys(&self, section: &str) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for l in &self.layers {
            if let Some(t) = l.table.get(section).and_then(|v| v.as_table()) {
                for k in t.keys() {
                    if !out.contains(&k.as_str()) {
                        out.push(k.as_str());
                    }
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// Las características encendidas, ordenadas.
    pub fn features_on(&self) -> Vec<&str> {
        self.keys("features").into_iter().filter(|k| self.on(k)).collect()
    }
}

/// Los estándares que hay para un lenguaje, ordenados por nombre de fichero.
///
/// No hay lista fija en Rust: se mira el directorio. Un dialecto nuevo
/// aparece aquí por existir.
pub fn standards_for(roots: &Roots, lang: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raiz in roots.paths() {
        let dir = raiz.join("standards").join(lang);
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "toml") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    if !out.iter().any(|x| x == stem) {
                        out.push(stem.to_string());
                    }
                }
            }
        }
    }
    out.sort();
    out
}

/// Los lenguajes que tienen tablas de estándares.
pub fn languages(roots: &Roots) -> Vec<String> {
    let mut out = Vec::new();
    for raiz in roots.paths() {
        let Ok(entries) = std::fs::read_dir(raiz.join("standards")) else { continue };
        for e in entries.flatten() {
            if e.path().is_dir() {
                if let Some(n) = e.file_name().to_str() {
                    if !out.iter().any(|x| x == n) {
                        out.push(n.to_string());
                    }
                }
            }
        }
    }
    out.sort();
    out
}

// ── Un módulo ───────────────────────────────────────────────────────────

/// Una función que un módulo ofrece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    /// La firma tal cual la escribió el autor (`"size_t -> ptr"`), si la puso.
    /// No se interpreta: es documentación con formato, y el día que alguien
    /// quiera comprobarla ya está aquí.
    pub signature: Option<String>,
}

/// Un `BMO.toml`.
///
/// `provides`/`requires` son los mismos nombres de capacidad que ya usa
/// `BMO_SYMBOLS.toml` en la raíz del repo (`storage.write`, `telemetry.log`).
/// Estaban en dos formatos distintos describiendo lo mismo; aquí caben en el
/// manifiesto para que un módulo declare qué NECESITA — que es la pregunta
/// que un sistema de capabilities tiene que poder hacerle antes de admitirlo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub exports: Vec<Export>,
    pub sources: Vec<String>,
    pub provides: Vec<String>,
    pub requires: Vec<String>,
}

impl Manifest {
    /// Lee un `BMO.toml` concreto.
    pub fn load(path: &Path) -> Result<Self, ModError> {
        let root = leer_toml(path)?;
        let mut m = Manifest::default();

        if let Some(t) = root.get("module").and_then(|v| v.as_table()) {
            m.name = t.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            m.version = t.get("version").and_then(|v| v.as_str()).unwrap_or("").to_string();
            m.provides = lista(t.get("provides"));
            m.requires = lista(t.get("requires"));
        }

        // `[exports]` admite las dos formas que ya hay escritas en el repo, y
        // no se elige una: romper los ocho manifiestos existentes para que la
        // gramática quede bonita es pagar con trabajo ajeno.
        //
        //   functions = "malloc, free"     ← lista de nombres
        //   malloc = "size_t -> ptr"       ← la clave ES el nombre
        if let Some(t) = root.get("exports").and_then(|v| v.as_table()) {
            for (k, v) in t {
                if k == "functions" {
                    for n in lista(Some(v)) {
                        m.exports.push(Export { name: n, signature: None });
                    }
                } else {
                    m.exports.push(Export {
                        name: k.clone(),
                        signature: v.as_str().map(str::to_string),
                    });
                }
            }
        }
        m.exports.sort_by(|a, b| a.name.cmp(&b.name));

        if let Some(t) = root.get("sources").and_then(|v| v.as_table()) {
            m.sources = lista(t.get("files"));
        }

        // Sin `[module] name`, el nombre es el del directorio. Un manifiesto
        // que no se nombra no es un error: se llama como su carpeta.
        if m.name.is_empty() {
            m.name = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
        }
        Ok(m)
    }

    /// Busca `<modulo>/BMO.toml` en las raíces. Devuelve también el
    /// directorio, que es donde están los fuentes.
    pub fn find(roots: &Roots, module: &str) -> Result<(Self, PathBuf), ModError> {
        let rel = format!("{module}/BMO.toml");
        let path = roots.locate_or_err(&rel)?;
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        Ok((Self::load(&path)?, dir))
    }

    pub fn export_names(&self) -> Vec<&str> {
        self.exports.iter().map(|e| e.name.as_str()).collect()
    }
}

/// Un valor TOML que puede ser una lista o una cadena con comas. Las dos
/// formas están escritas ya en el repo.
fn lista(v: Option<&toml::Value>) -> Vec<String> {
    match v {
        Some(toml::Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        Some(toml::Value::String(s)) => s
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots() -> Roots {
        let r = Roots::find();
        assert!(!r.is_empty(), "no encuentro las tablas desde {:?}", std::env::current_dir());
        r
    }

    /// El descubrimiento sube desde el cwd, así que tiene que funcionar igual
    /// desde la raíz del repo que desde el directorio de esta crate — que es
    /// donde cargo pone el cwd al correr los tests.
    #[test]
    fn encuentra_las_tablas_suba_desde_donde_suba() {
        let r = roots();
        assert!(r.locate("standards/C/c89.toml").is_some());
        assert!(r.locate("arch/x86_64/instructions.toml").is_some());
    }

    /// ★ Lo que no se podía hacer antes: C++ y COBOL tienen tablas escritas
    /// desde hace tiempo y NADIE las leía — el cargador estaba clavado a
    /// `standards/C`. Por la misma puerta, sin una línea de Rust por lenguaje.
    #[test]
    fn cualquier_lenguaje_entra_por_la_misma_puerta() {
        let r = roots();
        for (lang, name) in [("C", "c11"), ("CPLUSPLUS", "cpp17"), ("COBOL", "cobol85")] {
            let s = Standard::load(&r, lang, name).unwrap_or_else(|e| panic!("{lang}/{name}: {e}"));
            assert_eq!(s.lang(), lang);
            assert_eq!(s.name(), name);
        }
    }

    /// Las secciones son de verdad. El lector plano de antes metía
    /// `[features]`, `[type_rules]` y `[predefined_macros]` en el mismo saco:
    /// funcionaba porque ninguna clave se repetía, no porque estuviera bien.
    #[test]
    fn las_secciones_no_se_pisan() {
        let r = roots();
        let c11 = Standard::load(&r, "C", "c11").unwrap();
        assert!(c11.on("_Generic"), "C11 tiene _Generic");
        assert_eq!(c11.rule("implicit_int"), Some(false), "C11 lo elimino");
        // La misma clave preguntada a la seccion equivocada no contesta.
        assert_eq!(c11.flag("features", "implicit_int"), None);
        assert_eq!(c11.number("standard", "year"), Some(2011));
    }

    /// C89 es el caso que delató el bug histórico: cuando las rutas se
    /// quedaron muertas, el gating cayó al default en silencio y `//` pasaba
    /// a estar permitido en C89. Si esto se rompe, volvió a pasar.
    #[test]
    fn c89_sigue_sin_tener_comentarios_de_linea() {
        let r = roots();
        let c89 = Standard::load(&r, "C", "c89").unwrap();
        assert!(!c89.on("line_comments"));
        assert!(!c89.on("long_long"));
        assert_eq!(c89.rule("implicit_int"), Some(true));
    }

    /// ★ El defecto que las tablas llevaban dentro desde que se escribieron:
    /// declaran `[based_on] parent` y NADIE lo leía.
    ///
    /// `c17.toml` tiene `[features]` VACÍO —es una corrección de C11, no
    /// declara nada suyo— así que sin herencia C17 era un lenguaje con cero
    /// características. Y C23, que sólo lista lo que añade, había perdido
    /// `_Generic` y `_Atomic`.
    #[test]
    fn los_estandares_heredan_de_su_padre() {
        let r = roots();
        let c17 = Standard::load(&r, "C", "c17").unwrap();
        assert_eq!(c17.lineage(), vec!["c17", "c11", "c99", "c89"]);
        // Su propio fichero no dice ni una: todo esto viene de arriba.
        assert!(c17.on("_Generic"), "C17 es C11 corregido, tiene _Generic");
        assert!(c17.on("_Atomic"));
        assert!(c17.on("line_comments"), "de c99");
        assert!(c17.on("const"), "de c89");

        let c23 = Standard::load(&r, "C", "c23").unwrap();
        assert!(c23.on("nullptr"), "lo suyo");
        assert!(c23.on("_Generic"), "heredado de c11");
        assert!(c23.on("long_long"), "heredado de c99");
    }

    /// Y el hijo manda sobre el padre. `c89` permite el `int` implícito;
    /// `c99` lo prohíbe y hereda de `c89` — si ganara el padre, C99 volvería
    /// a permitirlo.
    #[test]
    fn el_hijo_pisa_al_padre() {
        let r = roots();
        let c99 = Standard::load(&r, "C", "c99").unwrap();
        assert_eq!(c99.rule("implicit_int"), Some(false));
        assert_eq!(c99.origin("type_rules", "implicit_int"), Some("c99"));
        assert!(c99.on("line_comments"));
        assert_eq!(c99.origin("features", "line_comments"), Some("c99"));
        // Lo que c99 no toca, lo pone c89 y se dice de dónde vino.
        assert!(c99.on("trigraphs"));
        assert_eq!(c99.origin("features", "trigraphs"), Some("c89"));
        // Y lo que no está en ninguna capa no tiene origen.
        assert_eq!(c99.origin("features", "nada_de_esto"), None);
    }

    /// ★ La tercera capa de las tres: un mod que sólo dice el DELTA.
    ///
    /// Es lo que separa extender de bifurcar. Este mod son cinco líneas y
    /// hereda C11 entero; el día que BMO corrija c11, se lleva la corrección.
    /// Copiar la tabla para cambiar una clave sí sería una bifurcación.
    #[test]
    fn un_mod_puede_ser_solo_el_delta_sobre_un_estandar() {
        let dir = tempdir("delta");
        let d = dir.join("standards/C");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("miempresa.toml"),
            "[standard]\nshort_name = \"MIEMPRESA\"\n\
             [features]\nsaturating_math = true\ntrigraphs = false\n\
             [based_on]\nparent = \"c11\"\n",
        )
        .unwrap();

        let mut paths = vec![dir.clone()];
        paths.extend(Roots::find().paths().iter().cloned());
        let r = Roots::from_paths(paths);

        let m = Standard::load(&r, "C", "miempresa").unwrap();
        assert_eq!(m.lineage(), vec!["miempresa", "c11", "c99", "c89"]);
        // Lo suyo.
        assert!(m.on("saturating_math"));
        // Heredado sin copiarlo.
        assert!(m.on("_Generic"));
        assert_eq!(m.rule("implicit_int"), Some(false));
        // Y puede APAGAR algo del padre, que es lo que hace util el delta.
        assert!(!m.on("trigraphs"));
        assert_eq!(m.origin("features", "trigraphs"), Some("miempresa"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Una cadena que se muerde la cola se para y se enseña entera. Sin esto
    /// el compilador se cuelga, y un compilador colgado no dice de quién es
    /// la culpa.
    #[test]
    fn la_herencia_circular_se_caza_en_vez_de_colgarse() {
        let dir = tempdir("ciclo");
        let d = dir.join("standards/C");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("ida.toml"), "[based_on]\nparent = \"vuelta\"\n").unwrap();
        std::fs::write(d.join("vuelta.toml"), "[based_on]\nparent = \"ida\"\n").unwrap();

        let r = Roots::from_paths(vec![dir.clone()]);
        let e = Standard::load(&r, "C", "ida").unwrap_err();
        let texto = format!("{e}");
        assert!(texto.contains("circular"), "{texto}");
        assert!(texto.contains("ida") && texto.contains("vuelta"), "{texto}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Un padre que no existe señala al padre, no al hijo. El autor del mod
    /// se equivocó escribiendo `parent`, y el mensaje tiene que llevarle ahí.
    #[test]
    fn un_padre_que_no_existe_lo_dice() {
        let dir = tempdir("huerfano");
        let d = dir.join("standards/C");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("huerfano.toml"), "[based_on]\nparent = \"c-que-no-hay\"\n").unwrap();

        let r = Roots::from_paths(vec![dir.clone()]);
        let texto = format!("{}", Standard::load(&r, "C", "huerfano").unwrap_err());
        assert!(texto.contains("c-que-no-hay"), "{texto}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Una clave que ningún Rust conoce se puede leer igual. Es la prueba de
    /// que declarar una extensión no necesita comité: `_Generic` no está en
    /// ninguna lista de este crate y aun así se pregunta por su nombre.
    #[test]
    fn una_clave_que_rust_no_conoce_se_lee_igual() {
        let r = roots();
        let c11 = Standard::load(&r, "C", "c11").unwrap();
        let encendidas = c11.features_on();
        assert!(encendidas.contains(&"_Atomic"));
        assert!(encendidas.contains(&"_Thread_local"));
        // Y lo que no está, no está — sin confundirlo con un error.
        assert!(!c11.on("una_extension_que_nadie_escribio"));
    }

    /// La lista de estándares sale del directorio, no de un `enum`.
    #[test]
    fn los_estandares_se_listan_mirando_el_disco() {
        let r = roots();
        let c = standards_for(&r, "C");
        assert!(c.contains(&"c89".to_string()) && c.contains(&"c23".to_string()), "{c:?}");
        let langs = languages(&r);
        for l in ["C", "COBOL", "CPLUSPLUS"] {
            assert!(langs.contains(&l.to_string()), "falta {l} en {langs:?}");
        }
    }

    /// Los ocho manifiestos que ya existen se leen con el formato nuevo. Si
    /// alguno no, el formato está mal — no el manifiesto.
    #[test]
    fn los_manifiestos_que_ya_existen_siguen_valiendo() {
        let r = roots();
        let (heap, dir) = Manifest::find(&r, "stdlib/heap").unwrap();
        assert!(heap.export_names().contains(&"malloc"));
        assert!(heap.export_names().contains(&"realloc"));
        assert!(dir.ends_with("stdlib/heap") || dir.ends_with("stdlib\\heap"));
        // La firma se conserva tal cual: es documentacion con formato.
        let malloc = heap.exports.iter().find(|e| e.name == "malloc").unwrap();
        assert_eq!(malloc.signature.as_deref(), Some("size_t -> ptr"));
    }

    /// Las dos formas de `[exports]` que hay escritas en el repo dan lo mismo.
    #[test]
    fn las_dos_formas_de_exports_valen() {
        let dir = tempdir("exports");
        let a = dir.join("clave-es-nombre");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::write(a.join("BMO.toml"), "[exports]\nmalloc = \"size_t -> ptr\"\nfree = \"ptr -> void\"\n").unwrap();
        let b = dir.join("lista");
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(b.join("BMO.toml"), "[exports]\nfunctions = \"malloc, free\"\n").unwrap();

        let ma = Manifest::load(&a.join("BMO.toml")).unwrap();
        let mb = Manifest::load(&b.join("BMO.toml")).unwrap();
        assert_eq!(ma.export_names(), mb.export_names());
        assert_eq!(ma.name, "clave-es-nombre", "sin [module] name, manda la carpeta");
    }

    /// ★ La prueba del mod de tercero: una tabla que NO está en el repo, en un
    /// directorio cualquiera, cargada por la misma puerta y **tapando** a la
    /// del sistema. Sin eso, "extensible" quiere decir "haz un fork".
    #[test]
    fn un_mod_de_fuera_se_carga_y_puede_tapar_al_sistema() {
        let dir = tempdir("mod-ajeno");
        let d = dir.join("standards/C");
        std::fs::create_dir_all(&d).unwrap();
        // Un dialecto que no existe en ninguna parte del repo.
        std::fs::write(
            d.join("c99-mio.toml"),
            "[standard]\nshort_name = \"C99-MIO\"\nyear = 2026\n\
             [features]\nline_comments = true\nmi_extension = true\n\
             [type_rules]\nimplicit_int = false\n",
        )
        .unwrap();
        // Y una copia de c89 que dice lo contrario que la del sistema.
        std::fs::write(d.join("c89.toml"), "[features]\nline_comments = true\n").unwrap();

        let sistema = Roots::find();
        let mut paths = vec![dir.clone()];
        paths.extend(sistema.paths().iter().cloned());
        let r = Roots::from_paths(paths);

        let mio = Standard::load(&r, "C", "c99-mio").unwrap();
        assert!(mio.on("mi_extension"), "una extension que Rust no conoce");
        assert_eq!(mio.text("standard", "short_name"), Some("C99-MIO"));

        // El de fuera gana: se puede corregir el sistema sin editarlo.
        let c89 = Standard::load(&r, "C", "c89").unwrap();
        assert!(c89.on("line_comments"), "la raiz de mods va primero");
        assert!(c89.path().starts_with(&dir));

        // Y aparece en la lista sin haber tocado ningun `enum`.
        assert!(standards_for(&r, "C").contains(&"c99-mio".to_string()));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Un mod ajeno mal escrito señala a su autor, con fichero y motivo. Un
    /// "no cargo" a secas manda a sospechar del sistema.
    #[test]
    fn un_toml_roto_dice_cual_y_por_que() {
        let dir = tempdir("roto");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("BMO.toml");
        std::fs::write(&f, "[module\nname = sin comillas").unwrap();
        let e = Manifest::load(&f).unwrap_err();
        let texto = format!("{e}");
        assert!(texto.contains("BMO.toml"), "{texto}");
        assert!(texto.contains("no es TOML valido"), "{texto}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Pedir algo que no está dice DÓNDE se miró. Sin eso, un mod que no
    /// carga se depura a ciegas.
    #[test]
    fn lo_que_no_esta_dice_donde_se_busco() {
        let r = roots();
        let e = Standard::load(&r, "C", "no-existe-jamas").unwrap_err();
        let texto = format!("{e}");
        assert!(texto.contains("no-existe-jamas"), "{texto}");
        assert!(texto.contains("standards/C"), "{texto}");
    }

    fn tempdir(etiqueta: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("bmo-mods-{etiqueta}-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
