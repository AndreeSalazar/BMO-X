//! **LO QUE UN PROGRAMA DECLARA QUE NECESITA**, y quien lo contesta.
//!
//! ## La linea que este modulo existe para poder leer
//!
//! ```text
//!     necesita monton 64 megas "los pesos del modelo viven en RAM"
//! ```
//!
//! ## *** Y LO QUE NO HACE, que es lo que lo distingue
//!
//! **No deduce nada.** El compilador no recorre el programa contando reservas
//! para adivinar cuanto monton hara falta. Lo dice el programa.
//!
//! Se podria haber intentado --sumar los literales, mirar los bucles-- y ese
//! camino tiene un final conocido: un numero que casi siempre acierta y que,
//! el dia que falla, falla sin que nadie sepa por que. Es la misma razon por la
//! que el perfil no se adivina y el `.bex` lo declara.
//!
//! ## Y de aqui salen DOS cosas, no una
//!
//! ```text
//!    el inmediato del arranque      cuanto se le pide al kernel al empezar
//!    la seccion `Requisitos`        lo que el CARGADOR lee antes de arrancar
//! ```
//!
//! ** La segunda es la que cambia el trato. Hasta el 2026-08-23 un programa que
//! necesitaba mas memoria de la que habia **arrancaba igual** y moria en su
//! primera reserva con un 1004. Con el requisito escrito, el cargador puede
//! negarse antes de la primera instruccion y **decir el motivo que escribio el
//! programa**, que es lo unico que convierte un rechazo en algo contestable.

use std::collections::HashMap;

use bmo_mods::Roots;

pub const RUTA: &str = "lang/inti/necesidades.toml";

const INCRUSTADA: &str =
    include_str!("../../../../forge/sem-asm/tables/lang/inti/necesidades.toml");

/// Una clase de las que se pueden pedir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clase {
    /// El numero del ABI (`bmo_abi::bef::requisitos::CLASE_*`).
    pub numero: u16,
    /// 0 = unidades, 1 = bytes. Los mismos de `UNIDAD_*`.
    pub unidad: u16,
}

/// La tabla, ya leida.
#[derive(Debug, Clone)]
pub struct Necesidades {
    monton_por_defecto: u64,
    monton_maximo: u64,
    unidades: HashMap<String, u64>,
    clases: HashMap<String, Clase>,
}

impl Default for Necesidades {
    fn default() -> Self {
        Self::por_defecto()
    }
}

impl Necesidades {
    pub fn por_defecto() -> Self {
        Self::desde_texto(INCRUSTADA)
    }

    pub fn cargar(raices: &Roots) -> Self {
        match raices.locate(RUTA).and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(t) => Self::desde_texto(&t),
            None => Self::por_defecto(),
        }
    }

    /// Lo que dice ESTE texto de tabla.
    ///
    /// ** Una tabla rota no revienta el compilador: deja los mapas vacios, y
    /// entonces toda unidad y toda clase son desconocidas. El programa se entera
    /// con un aviso en su linea, que es donde se puede hacer algo -- y no con un
    /// panico que habla de una instalacion que el usuario no ha tocado.
    pub fn desde_texto(t: &str) -> Self {
        let doc: toml::Value = match t.parse() {
            Ok(v) => v,
            Err(_) => toml::Value::Table(Default::default()),
        };
        let leer_u64 = |seccion: &str, clave: &str, si_no: u64| -> u64 {
            doc.get(seccion)
                .and_then(|s| s.get(clave))
                .and_then(|v| v.as_integer())
                .filter(|n| *n >= 0)
                .map(|n| n as u64)
                .unwrap_or(si_no)
        };

        let mut unidades = HashMap::new();
        if let Some(tabla) = doc.get("unidades").and_then(|v| v.as_table()) {
            for (nombre, valor) in tabla {
                if let Some(n) = valor.as_integer().filter(|n| *n > 0) {
                    unidades.insert(nombre.clone(), n as u64);
                }
            }
        }

        let mut clases = HashMap::new();
        if let Some(tabla) = doc.get("clases").and_then(|v| v.as_table()) {
            for (nombre, valor) in tabla {
                let numero = valor.get("numero").and_then(|v| v.as_integer());
                let unidad = valor.get("unidad").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(n) = numero.filter(|n| *n > 0 && *n <= u16::MAX as i64) {
                    clases.insert(
                        nombre.clone(),
                        Clase {
                            numero: n as u16,
                            unidad: if unidad == "bytes" { 1 } else { 0 },
                        },
                    );
                }
            }
        }

        Necesidades {
            monton_por_defecto: leer_u64("monton", "por_defecto", 4096),
            monton_maximo: leer_u64("monton", "maximo", 0),
            unidades,
            clases,
        }
    }

    /// El monton de una tarea que no dijo nada.
    pub fn monton_por_defecto(&self) -> u64 {
        self.monton_por_defecto
    }

    /// El techo. Cero quiere decir *"la tabla no lo dice"*, y entonces no hay.
    pub fn monton_maximo(&self) -> u64 {
        self.monton_maximo
    }

    /// Cuantos bytes es una unidad, si existe.
    pub fn unidad(&self, nombre: &str) -> Option<u64> {
        self.unidades.get(nombre).copied()
    }

    /// La clase con ese nombre, si existe.
    pub fn clase(&self, nombre: &str) -> Option<Clase> {
        self.clases.get(nombre).copied()
    }

    /// Los nombres que SI valen, ordenados. Para que el aviso no solo diga que
    /// no, sino cuales son.
    pub fn clases_conocidas(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.clases.keys().map(|s| s.as_str()).collect();
        v.sort_unstable();
        v
    }

    pub fn unidades_conocidas(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.unidades.keys().map(|s| s.as_str()).collect();
        v.sort_unstable();
        v
    }
}

pub mod revisa;
pub use revisa::{monton_de, revisa, Pedido};

#[cfg(test)]
mod pruebas;
