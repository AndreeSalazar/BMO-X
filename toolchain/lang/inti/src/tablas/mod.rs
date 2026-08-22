//! `tablas` -- los datos agnosticos que varias piezas necesitan leer.
//!
//! ## Por que existe este modulo, y quien lo pidio
//!
//! No lo pidio un diseno: lo pidio `tests/linaje.rs`.
//!
//! `Modulos` --que nombres trae cada `usa`, que recoge cada uno de la puerta,
//! que ancho tiene cada acceso a memoria-- vivia dentro de `nombres` porque
//! `nombres` fue lo primero que la necesito. Cuando `perfil` y `ir` tambien la
//! necesitaron, el test paro la compilacion con la frase exacta:
//!
//! > *ir mira a su hermano nombres: son dos piezas o una, no las dos cosas*
//!
//! Tenia razon. Tres hermanos agarrados a una tabla que vive dentro de uno de
//! ellos **no son tres piezas**: son una con tres nombres, y el dia que haya que
//! reescribir `nombres` se caen los tres.
//!
//! ## ** La regla que sale de esto, y vale para la siguiente tabla
//!
//! > **Una tabla vive en la generacion mas baja que la necesita.**
//!
//! Por eso `Comun` NO se mudo: la lee `nombres` y nadie mas, asi que su sitio
//! sigue siendo `nombres`. No es incoherencia -- es la misma regla dando otra
//! respuesta porque el dato es otro. El dia que `perfil` necesite `Comun`, el
//! test lo dira y se mudara tambien.
//!
//! ## Lo que este modulo NO puede saber
//!
//! Ninguna maquina. Todo lo de aqui es del ABI de BMO-X o del lenguaje: que
//! `invoca_valor` recoge el valor es verdad en toda maquina, y DONDE esta ese
//! valor no se pregunta aqui. Eso vive en `arquitectura`, que es hermano de
//! generacion y no se mira con este.

use std::collections::{HashMap, HashSet};

use bmo_mods::Roots;

pub const RUTA_MODULOS: &str = "lang/inti/modulos.toml";

const INCRUSTADOS: &str =
    include_str!("../../../../forge/sem-asm/tables/lang/inti/modulos.toml");

/// Que nombres trae cada `usa <modulo>` de REX.
///
/// Es la tercera tabla del reparto, y la linea que las separa es una sola
/// pregunta: **lo escribe casi todo el mundo?** Si si, va en `comun.toml`; si
/// lo escriben algunos, es un modulo y hay que pedirlo.
#[derive(Debug, Clone, Default)]
pub struct Modulos {
    por_nombre: HashMap<String, Vec<String>>,
    /// nombre -> ("lee"|"escribe", ancho en bytes).
    ///
    /// ** El ancho va en BYTES y no en el nombre de la instruccion. "8" es
    /// verdad en toda maquina; "qword" solo en una.
    accede: HashMap<String, (String, u32)>,
    /// nombre -> tiene que ir dentro de `crudo`.
    ///
    /// ** El `crudo` viaja con quien trae el nombre: la maquina trae
    /// `entrada_puerto` y su prohibicion, el modulo `memoria` trae
    /// `escribe_natural64` y la suya.
    pide_crudo: HashSet<String>,
    /// nombre -> su valor. Numeros del ABI de BMO-X, no de ninguna maquina.
    constantes: HashMap<String, u64>,
    /// nombre -> que recoge de la puerta ("codigo" o "valor").
    ///
    /// ** No es un detalle de emision que se pueda dejar para luego: leer el
    /// registro equivocado no falla, DEVUELVE OTRA COSA. Y las dos cosas son
    /// numeros del mismo ancho, asi que nada se queja.
    recoge: HashMap<String, String>,
    /// Los modulos cuyos nombres se bajan a UNA INSTRUCCION, no a una llamada.
    ///
    /// ** Sin esto, `cuenta_unos(x)` se bajaba a un `call` a un simbolo que no
    /// existe: compilaba, pasaba el analisis de nombres --porque el nombre esta
    /// en la tabla-- y el binario saltaba a la nada.
    instrucciones: HashSet<String>,
}

impl Modulos {
    pub fn por_defecto() -> Self {
        Self::desde_texto(INCRUSTADOS)
    }

    pub fn cargar(raices: &Roots) -> Self {
        match raices
            .locate(RUTA_MODULOS)
            .and_then(|p| std::fs::read_to_string(p).ok())
        {
            Some(t) => Self::desde_texto(&t),
            None => Self::por_defecto(),
        }
    }

    fn desde_texto(t: &str) -> Self {
        let raiz: toml::Value = match t.parse() {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let mut por_nombre = HashMap::new();
        let mut recoge = HashMap::new();
        let mut accede = HashMap::new();
        let mut pide_crudo = HashSet::new();
        let mut constantes = HashMap::new();
        let mut instrucciones = HashSet::new();
        if let Some(tabla) = raiz.as_table() {
            for (k, v) in tabla {
                // `recoge` no es un modulo: nadie escribe `usa recoge`. Es una
                // columna mas sobre nombres que ya trae otro.
                // Estas no son modulos: nadie escribe `usa recoge`. Son
                // columnas mas sobre nombres que ya trae otro.
                if matches!(
                    k.as_str(),
                    "meta" | "recoge" | "accede" | "crudo" | "constantes" | "instrucciones"
                ) {
                    continue;
                }
                if let Some(t) = v.as_table() {
                    por_nombre.insert(k.clone(), t.keys().cloned().collect());
                }
            }
            if let Some(t) = raiz.get("recoge").and_then(|v| v.as_table()) {
                for (k, v) in t {
                    if let Some(q) = v.as_str() {
                        recoge.insert(k.clone(), q.to_string());
                    }
                }
            }
            if let Some(t) = raiz.get("accede").and_then(|v| v.as_table()) {
                for (k, v) in t {
                    let hace = v.get("hace").and_then(|x| x.as_str());
                    let bytes = v.get("bytes").and_then(|x| x.as_integer());
                    // Una fila a medias se tira entera. Un acceso con ancho
                    // inventado leeria un numero que no es, sin quejarse.
                    if let (Some(h), Some(b)) = (hace, bytes) {
                        accede.insert(k.clone(), (h.to_string(), b as u32));
                    }
                }
            }
            if let Some(a) = raiz
                .get("crudo")
                .and_then(|c| c.get("piden"))
                .and_then(|v| v.as_array())
            {
                pide_crudo.extend(a.iter().filter_map(|x| x.as_str().map(String::from)));
            }
            if let Some(a) = raiz
                .get("instrucciones")
                .and_then(|c| c.get("son"))
                .and_then(|v| v.as_array())
            {
                instrucciones.extend(a.iter().filter_map(|x| x.as_str().map(String::from)));
            }
            if let Some(t) = raiz.get("constantes").and_then(|v| v.as_table()) {
                for (k, v) in t {
                    if let Some(x) = v.as_str() {
                        let limpio = x.trim_start_matches("0x").trim_start_matches("0X");
                        if let Ok(n) = u64::from_str_radix(limpio, 16) {
                            constantes.insert(k.clone(), n);
                        }
                    }
                }
            }
        }
        Self {
            por_nombre,
            recoge,
            accede,
            pide_crudo,
            constantes,
            instrucciones,
        }
    }

    /// Los nombres de este modulo, se bajan a una instruccion?
    ///
    /// ** La diferencia no es cosmetica: una funcion se llama y una instruccion
    /// se emite. Bajar una instruccion como llamada produce un salto a un
    /// simbolo que no existe -- y eso **compila**.
    pub fn son_instrucciones(&self, modulo: &str) -> bool {
        self.instrucciones.contains(modulo)
    }

    /// Que acceso a memoria es este nombre, y de que ancho en bytes.
    pub fn accede(&self, nombre: &str) -> Option<(&str, u32)> {
        self.accede.get(nombre).map(|(h, b)| (h.as_str(), *b))
    }

    /// Si este nombre, venga del modulo que venga, tiene que ir en `crudo`.
    pub fn pide_crudo(&self, nombre: &str) -> bool {
        self.pide_crudo.contains(nombre)
    }

    /// El valor de una constante del ABI.
    pub fn constante(&self, nombre: &str) -> Option<u64> {
        self.constantes.get(nombre).copied()
    }

    /// Que recoge este nombre de la puerta: `"codigo"` o `"valor"`.
    ///
    /// `None` si el nombre no cruza ninguna puerta, que es lo normal.
    pub fn recoge(&self, nombre: &str) -> Option<&str> {
        self.recoge.get(nombre).map(|s| s.as_str())
    }

    /// Los nombres que trae un `usa`. Vacio si no es un modulo conocido -- que
    /// no es un error: puede ser una arquitectura, y esa la resuelve otro.
    pub fn trae(&self, modulo: &str) -> &[String] {
        self.por_nombre
            .get(modulo)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

/// Las piezas de INTI que estan escritas EN INTI.
///
/// ## ** Por que el monton no esta en Rust
///
/// Porque `llano` presume de poder escribir el sistema, y la forma de
/// demostrarlo no es repetirlo: es **escribir en `llano` la pieza que hace
/// posible `pleno`**. Si el monton hubiera que escribirlo en Rust, la frase
/// "INTI puede escribir el sistema" seria publicidad.
///
/// Y trae dos cosas gratis que no se pueden fingir:
///
/// - sus bloques `crudo` **se cuentan**, porque pasan por el mismo analisis que
///   los de cualquier programa;
/// - vive en `tables/`, asi que **`$BMO_MODS` puede sustituirlo sin bifurcar el
///   compilador**. Cambiar el repartidor de memoria del lenguaje es dejar otro
///   fichero delante.
///
/// ## Como se busca
///
/// ```text
///    lang/inti/runtime/<nombre>/          una carpeta de piezas, en orden
///    lang/inti/runtime/<nombre>.inti      o una pieza sola
/// ```
///
/// La carpeta va primero a proposito: **lo modular es el caso normal**, y el
/// fichero suelto es la excepcion para lo que no da para dos piezas.
pub struct Runtime;

impl Runtime {
    /// Lo que trae un `usa <nombre>` que sea una pieza escrita en INTI.
    ///
    /// Vacio si no lo es, que no es un error: puede ser una maquina, o un
    /// modulo de REX.
    pub fn traer(raices: &Roots, nombre: &str) -> Vec<(String, String)> {
        // Un nombre con separadores buscaria fuera del sitio. No se limpia --
        // se rechaza: un `usa ../../algo` que "casi" funciona es peor que uno
        // que no existe.
        if nombre.is_empty() || !nombre.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Vec::new();
        }

        if let Some(dir) = raices.locate(&format!("lang/inti/runtime/{}", nombre)) {
            if dir.is_dir() {
                let mut piezas: Vec<(String, String)> = std::fs::read_dir(&dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| e.path().extension().is_some_and(|x| x == "inti"))
                    .filter_map(|e| {
                        let n = e.file_name().to_string_lossy().to_string();
                        std::fs::read_to_string(e.path()).ok().map(|t| (n, t))
                    })
                    .collect();
                // Por nombre de fichero, para que dos compilaciones de la misma
                // fuente den el mismo binario. El orden de `read_dir` lo elige
                // el sistema de ficheros, y eso no es una fuente.
                piezas.sort_by(|a, b| a.0.cmp(&b.0));
                return piezas;
            }
        }

        raices
            .locate(&format!("lang/inti/runtime/{}.inti", nombre))
            .and_then(|p| std::fs::read_to_string(&p).ok().map(|t| (nombre.to_string(), t)))
            .into_iter()
            .collect()
    }
}
