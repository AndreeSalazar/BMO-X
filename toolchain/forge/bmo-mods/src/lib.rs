//! **El contrato de los mods de BMO** -- formato, nunca cerebro.
//!
//! Libreria que cada frontend ELIGE enlazar, como el resto de `forge/`. No es
//! un embudo: quien no la quiera sigue leyendo sus ficheros a mano.
//!
//! ## Que problema resuelve
//!
//! Un comite decide que entra en un lenguaje y cuando. Eso da estabilidad y
//! cuesta anos por cada cambio. La alternativa que ya practica este repo --
//! *"anadir una instruccion = 1 entrada TOML, CERO Rust"*-- no necesita comite:
//! el que quiere una extension la declara y la usa.
//!
//! Lo que faltaba no era la idea, era **un solo formato**. Habia tres:
//!
//! - `lang/c/src/module.rs` leia `BMO.toml` partiendo lineas por `=`.
//! - `lang/c/src/standard.rs` leia `standards/C/*.toml` igual, y **se comia
//!   las secciones**: `[features]` y `[type_rules]` caian en el mismo saco.
//!   Funcionaba por suerte, porque ninguna clave se repetia todavia.
//! - `forge/sem-asm` si usaba un parser de verdad, para otras tablas.
//!
//! Y los dos primeros llevaban copiada la misma lista de cinco rutas
//! candidatas para encontrar `tables/`. Cuando esa lista se quedo vieja, el
//! gating de estandares **cayo al default en silencio** durante meses -- esta
//! escrito en el comentario de `standard.rs`. Un formato con tres lectores
//! tiene tres formas de mentir.
//!
//! ## La frontera honesta
//!
//! Esto quita el Rust de **DECLARAR** una extension, no de implementarla.
//! Anadir `mi_extension = true` a un TOML es gratis; que el compilador haga
//! algo distinto sigue siendo codigo. Es la misma frontera que la fabrica de
//! COBOL: lo tabular se genera, la semantica de cada verbo se escribe.
//!
//! Prometer mas que eso seria vender compatibilidad que no existe.
//!
//! ## Donde se buscan los mods
//!
//! En este orden, y el primero que tenga el fichero gana:
//!
//! 1. `$BMO_MODS` -- una o varias rutas separadas por `;`. **Es la puerta de
//!    los terceros**: se dejan ahi y no se toca el repo. Va primero a
//!    proposito, para poder tapar una tabla del sistema sin editarla.
//! 2. `mods/` en la raiz del repo, si existe.
//! 3. `toolchain/forge/sem-asm/tables/` -- las tablas del sistema.
//!
//! ## Lo que esto NO hace, y hay que decirlo
//!
//! Un mod de tabla es **datos**: no ejecuta nada, asi que no puede robar
//! nada. El dia que un mod sea un `.bex` con codigo, esto no basta: haria
//! falta el gate de firma y `bmo-verify` -- que hoy esta escrito y **no tiene
//! un solo usuario**. Ese es el orden correcto y por eso se empieza por aqui.

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ModError {
    /// No se encontro ninguna raiz con tablas. Casi siempre es el cwd.
    SinRaices,
    /// El fichero no esta en ninguna de las raices.
    NoEsta { que: String, buscado_en: Vec<PathBuf> },
    /// El fichero esta pero no se puede leer.
    NoSeLee(PathBuf),
    /// El fichero esta y no es TOML valido. Se dice CUAL y POR QUE: un mod
    /// ajeno mal escrito tiene que senalar a su autor, no al sistema.
    NoEsToml { fichero: PathBuf, motivo: String },
    /// Una cadena de herencia que se muerde la cola. Se para y se ensena
    /// entera: `a -> b -> a` colgaria el compilador, y un compilador colgado no
    /// dice de quien es la culpa.
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

// -- Donde vive todo -----------------------------------------------------

/// Las raices donde se buscan tablas y modulos, en orden de prioridad.
///
/// Existe para que la lista de rutas candidatas este **en un sitio**. Estaba
/// copiada en dos, y cuando el repo se reorganizo una de las copias apunto
/// durante meses a un directorio muerto sin que nadie se enterara: el
/// descubrimiento fallaba devolviendo "no hay", que se parece demasiado a
/// "no hace falta".
#[derive(Debug, Clone)]
pub struct Roots {
    paths: Vec<PathBuf>,
}

impl Roots {
    /// Descubre las raices. Nunca falla: puede devolver una lista vacia, y
    /// entonces cada `load` dira exactamente donde miro.
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

        // 2 y 3. La raiz del repo, buscada hacia arriba desde el cwd. Subir
        //    en vez de listar rutas relativas a ojo: asi funciona igual desde
        //    la raiz, desde `toolchain/lang/c` o desde donde sea.
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

    /// Construye unas raices explicitas. Para los tests y para quien quiera
    /// mandar el.
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
    /// no vale --hay uno en cada crate--; la marca es el par raiz.
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

// -- Un estandar (o un dialecto de nadie) --------------------------------

/// Una capa de la cadena de herencia: un fichero y de donde salio.
#[derive(Debug, Clone)]
struct Layer {
    name: String,
    path: PathBuf,
    table: toml::Table,
}

/// Un fichero de `standards/<LENGUAJE>/<name>.toml`, **con su herencia
/// resuelta**.
///
/// Se llama "estandar" porque hoy los que hay son de comite (C11, COBOL85,
/// C++17), pero el formato no distingue: un dialecto propio en
/// `standards/C/mio.toml` se carga por la misma puerta y con el mismo peso.
/// **Esa es la idea entera.**
///
/// ## Las tres capas, y por que son tres
///
/// El mismo mecanismo da tres posturas distintas, y no hace falta un
/// mecanismo por postura:
///
/// | Quiero... | Se escribe |
/// |---|---|
/// | el estandar de BMO tal cual | nada |
/// | **mi propio estandar** | una tabla SIN `parent` |
/// | **anadir cosas a uno** | una tabla CON `parent` y solo el delta |
///
/// La tercera es la que evita que esto sea anarquia. Un mod que declara
/// `parent = "c11"` y tres claves **no puede bifurcar el resto**: el dia que
/// BMO corrige c11, ese mod se lleva la correccion. Copiar la tabla entera
/// para cambiar tres lineas si es una bifurcacion, y es lo que habia que
/// hacer antes de esto.
///
/// ## Las capas no se funden
///
/// Se guardan en orden hijo->padre y cada consulta baja hasta el primero que
/// contesta. Cuesta lo mismo y permite `origin()`: saber **que fichero** puso
/// un valor. En un sistema donde cualquiera puede tapar una tabla, "de donde
/// ha salido esto?" es la primera pregunta que se hace todo el mundo.
///
/// ## Las caracteristicas no son un `struct` de Rust
///
/// Antes lo eran, con un `match` de once claves: anadir una exigia tocar tres
/// sitios de Rust --el campo, el `Default` y el `match`--, que es exactamente el
/// tramite de comite del que se queria salir. Aqui se preguntan por nombre y
/// una clave nueva no necesita recompilar nada.
#[derive(Debug, Clone)]
pub struct Standard {
    lang: String,
    layers: Vec<Layer>,
}

/// Tope de la cadena. Existe para que un error de escritura no se lleve por
/// delante la pila; el ciclo lo caza la lista de visitados, esto es el cinturon.
const MAX_HERENCIA: usize = 32;

impl Standard {
    /// Carga `standards/<lang>/<name>.toml` y su cadena de `[based_on] parent`.
    ///
    /// El padre se busca por las MISMAS raices, asi que un mod puede heredar
    /// de una tabla del sistema -- y si alguien tapo esa tabla, hereda de la
    /// tapada. Es lo coherente: una raiz que va primero, va primero siempre.
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

    /// De que fichero salio. Un mensaje de error que no dice esto obliga a
    /// adivinar cual de las raices gano.
    pub fn path(&self) -> &Path {
        &self.layers[0].path
    }

    /// La cadena de herencia, del hijo al ancestro: `["c23","c17","c11",...]`.
    pub fn lineage(&self) -> Vec<&str> {
        self.layers.iter().map(|l| l.name.as_str()).collect()
    }

    /// Que tabla de la cadena puso este valor. `None` si nadie lo pone.
    ///
    /// Es la respuesta a "por que mi flag no hace efecto?" -- casi siempre
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

    /// Esta encendida esta caracteristica en `[features]`?
    ///
    /// Ausente es `false`, y no es lo mismo que un error: un estandar viejo
    /// simplemente no menciona lo que aun no existia.
    pub fn on(&self, feature: &str) -> bool {
        self.flag("features", feature).unwrap_or(false)
    }

    /// Una regla de `[type_rules]`. Aqui `None` SI importa y por eso no se
    /// aplana a `false`: "C89 permite `int` implicito" y "esta tabla no dice
    /// nada del `int` implicito" llevan a compiladores distintos.
    pub fn rule(&self, key: &str) -> Option<bool> {
        self.flag("type_rules", key)
    }

    /// Un booleano de cualquier seccion. La seccion importa: leer el TOML
    /// plano juntaba `[features]` con `[type_rules]` y con
    /// `[predefined_macros]`, y bastaba una clave repetida entre secciones
    /// para que una pisara a la otra.
    pub fn flag(&self, section: &str, key: &str) -> Option<bool> {
        self.lookup(section, key)?.as_bool()
    }

    /// Un texto de cualquier seccion (`[standard].iso_number`, ...).
    ///
    /// OJO: `[standard]` tambien se hereda, asi que un mod que no se ponga
    /// `short_name` ensena el de su padre. Es deliberado -- un mod que solo
    /// anade tres claves no tiene por que reescribir la ficha entera.
    pub fn text(&self, section: &str, key: &str) -> Option<&str> {
        self.lookup(section, key)?.as_str()
    }

    /// Un entero de cualquier seccion (`[standard].year`, ...).
    pub fn number(&self, section: &str, key: &str) -> Option<i64> {
        self.lookup(section, key)?.as_integer()
    }

    /// Todas las claves de una seccion **de toda la cadena**, ordenadas y sin
    /// repetir. Con esto un frontend puede recorrer lo que la tabla trae
    /// **sin conocerlo de antemano** -- que es lo que hace falta para que un
    /// mod anada algo que el compilador no tenia previsto.
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

    /// Las caracteristicas encendidas, ordenadas.
    pub fn features_on(&self) -> Vec<&str> {
        self.keys("features").into_iter().filter(|k| self.on(k)).collect()
    }
}

/// Los estandares que hay para un lenguaje, ordenados por nombre de fichero.
///
/// No hay lista fija en Rust: se mira el directorio. Un dialecto nuevo
/// aparece aqui por existir.
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

/// Los lenguajes que tienen tablas de estandares.
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

// -- Un modulo -----------------------------------------------------------

/// Una funcion que un modulo ofrece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    /// La firma tal cual la escribio el autor (`"size_t -> ptr"`), si la puso.
    /// No se interpreta: es documentacion con formato, y el dia que alguien
    /// quiera comprobarla ya esta aqui.
    pub signature: Option<String>,
}

/// Un `BMO.toml`.
///
/// `provides`/`requires` son los mismos nombres de capacidad que ya usa
/// `BMO_SYMBOLS.toml` en la raiz del repo (`storage.write`, `telemetry.log`).
/// Estaban en dos formatos distintos describiendo lo mismo; aqui caben en el
/// manifiesto para que un modulo declare que NECESITA -- que es la pregunta
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
        // gramatica quede bonita es pagar con trabajo ajeno.
        //
        //   functions = "malloc, free"     <- lista de nombres
        //   malloc = "size_t -> ptr"       <- la clave ES el nombre
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

    /// Busca `<modulo>/BMO.toml` en las raices. Devuelve tambien el
    /// directorio, que es donde estan los fuentes.
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
/// formas estan escritas ya en el repo.
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

    /// El descubrimiento sube desde el cwd, asi que tiene que funcionar igual
    /// desde la raiz del repo que desde el directorio de esta crate -- que es
    /// donde cargo pone el cwd al correr los tests.
    #[test]
    fn encuentra_las_tablas_suba_desde_donde_suba() {
        let r = roots();
        assert!(r.locate("standards/C/c89.toml").is_some());
        assert!(r.locate("arch/x86_64/instructions.toml").is_some());
    }

    /// * Lo que no se podia hacer antes: C++ y COBOL tienen tablas escritas
    /// desde hace tiempo y NADIE las leia -- el cargador estaba clavado a
    /// `standards/C`. Por la misma puerta, sin una linea de Rust por lenguaje.
    #[test]
    fn cualquier_lenguaje_entra_por_la_misma_puerta() {
        let r = roots();
        for (lang, name) in [("C", "c11"), ("CPLUSPLUS", "cpp17"), ("COBOL", "cobol85")] {
            let s = Standard::load(&r, lang, name).unwrap_or_else(|e| panic!("{lang}/{name}: {e}"));
            assert_eq!(s.lang(), lang);
            assert_eq!(s.name(), name);
        }
    }

    /// Las secciones son de verdad. El lector plano de antes metia
    /// `[features]`, `[type_rules]` y `[predefined_macros]` en el mismo saco:
    /// funcionaba porque ninguna clave se repetia, no porque estuviera bien.
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

    /// C89 es el caso que delato el bug historico: cuando las rutas se
    /// quedaron muertas, el gating cayo al default en silencio y `//` pasaba
    /// a estar permitido en C89. Si esto se rompe, volvio a pasar.
    #[test]
    fn c89_sigue_sin_tener_comentarios_de_linea() {
        let r = roots();
        let c89 = Standard::load(&r, "C", "c89").unwrap();
        assert!(!c89.on("line_comments"));
        assert!(!c89.on("long_long"));
        assert_eq!(c89.rule("implicit_int"), Some(true));
    }

    /// * El defecto que las tablas llevaban dentro desde que se escribieron:
    /// declaran `[based_on] parent` y NADIE lo leia.
    ///
    /// `c17.toml` tiene `[features]` VACIO --es una correccion de C11, no
    /// declara nada suyo-- asi que sin herencia C17 era un lenguaje con cero
    /// caracteristicas. Y C23, que solo lista lo que anade, habia perdido
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

    /// Y el hijo manda sobre el padre. `c89` permite el `int` implicito;
    /// `c99` lo prohibe y hereda de `c89` -- si ganara el padre, C99 volveria
    /// a permitirlo.
    #[test]
    fn el_hijo_pisa_al_padre() {
        let r = roots();
        let c99 = Standard::load(&r, "C", "c99").unwrap();
        assert_eq!(c99.rule("implicit_int"), Some(false));
        assert_eq!(c99.origin("type_rules", "implicit_int"), Some("c99"));
        assert!(c99.on("line_comments"));
        assert_eq!(c99.origin("features", "line_comments"), Some("c99"));
        // Lo que c99 no toca, lo pone c89 y se dice de donde vino.
        assert!(c99.on("trigraphs"));
        assert_eq!(c99.origin("features", "trigraphs"), Some("c89"));
        // Y lo que no esta en ninguna capa no tiene origen.
        assert_eq!(c99.origin("features", "nada_de_esto"), None);
    }

    /// * La tercera capa de las tres: un mod que solo dice el DELTA.
    ///
    /// Es lo que separa extender de bifurcar. Este mod son cinco lineas y
    /// hereda C11 entero; el dia que BMO corrija c11, se lleva la correccion.
    /// Copiar la tabla para cambiar una clave si seria una bifurcacion.
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

    /// Una cadena que se muerde la cola se para y se ensena entera. Sin esto
    /// el compilador se cuelga, y un compilador colgado no dice de quien es
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

    /// Un padre que no existe senala al padre, no al hijo. El autor del mod
    /// se equivoco escribiendo `parent`, y el mensaje tiene que llevarle ahi.
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

    /// Una clave que ningun Rust conoce se puede leer igual. Es la prueba de
    /// que declarar una extension no necesita comite: `_Generic` no esta en
    /// ninguna lista de este crate y aun asi se pregunta por su nombre.
    #[test]
    fn una_clave_que_rust_no_conoce_se_lee_igual() {
        let r = roots();
        let c11 = Standard::load(&r, "C", "c11").unwrap();
        let encendidas = c11.features_on();
        assert!(encendidas.contains(&"_Atomic"));
        assert!(encendidas.contains(&"_Thread_local"));
        // Y lo que no esta, no esta -- sin confundirlo con un error.
        assert!(!c11.on("una_extension_que_nadie_escribio"));
    }

    /// La lista de estandares sale del directorio, no de un `enum`.
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
    /// alguno no, el formato esta mal -- no el manifiesto.
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

    /// * La prueba del mod de tercero: una tabla que NO esta en el repo, en un
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

    /// Un mod ajeno mal escrito senala a su autor, con fichero y motivo. Un
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

    /// Pedir algo que no esta dice DONDE se miro. Sin eso, un mod que no
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
