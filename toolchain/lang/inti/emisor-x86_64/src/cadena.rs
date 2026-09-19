//! **La cadena entera, en una llamada**: del texto de un `.inti` a los bytes
//! de un `.ibx` que ya paso el gate.
//!
//! ## Por que existe (2026-09-19)
//!
//! Los seis pasos --revisar, armar, disponer, bajar, emitir, empaquetar--
//! estaban escritos a mano en `main.rs`, y el metro del emisor
//! (`toolchain/tools/metro`) queria compilar INTI **por el mismo camino que
//! la linea de ordenes**. Copiar los seis pasos alli seria medir otro
//! compilador: la regla que el banco se puso en F2d y que `main.rs` repite.
//! Asi que la cadena sale a una funcion y `main.rs` la llama igual que el
//! metro.
//!
//! Lo que devuelve es lo que `main.rs` necesita para su informe (`-i`): el
//! parte de CABINA, lo emitido y cuantos eventos hubo. Lo que falla se
//! devuelve PINTADO, con el mismo texto que salia por la consola.

use std::path::Path;

use crate::{empaquetar, emitir, Emitido};

/// Lo que sale de la cadena cuando todo fue bien.
pub struct Compilado {
    /// El `.ibx`: ya paso por `bmo-verify`.
    pub bytes: Vec<u8>,
    pub emitido: Emitido,
    pub parte: bmo_inti_front::cabina::Parte,
    pub eventos: usize,
}

/// Por que no salio un `.ibx`. Cada brazo lleva el texto que se pinta.
#[derive(Debug)]
pub enum Fallo {
    /// Avisos de error del frontend, ya pintados con su fichero.
    Avisos(String),
    /// Cosas que se pidieron y no llegaron a un byte (E0075).
    SinEmitir(Vec<String>),
    /// El gate de `empaquetar` dijo que no.
    Gate(String),
}

impl std::fmt::Display for Fallo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fallo::Avisos(t) => write!(f, "{t}"),
            Fallo::SinEmitir(v) => write!(f, "E0075 {} cosa(s) se pidieron y no llegaron a un byte: {}", v.len(), v.join("; ")),
            Fallo::Gate(e) => write!(f, "el `.bex` no pasa el gate: {e}"),
        }
    }
}

/// Compila `texto` (el fuente de `nombre`) hasta el `.ibx`.
///
/// `raices` son las tablas (`bmo_mods::Roots::find()` desde la linea de
/// ordenes); se pasan para que quien llame varias veces las cargue una.
pub fn compilar(texto: &str, nombre: &str, raices: &bmo_mods::Roots) -> Result<Compilado, Fallo> {
    // ** Por `informar` y no montando los analisis a mano: si esto compilara
    // por otro camino, estaria probando otro compilador.
    let (parte, eventos) = bmo_inti_front::informar(texto, nombre);

    // ** LOS AVISOS SALEN DE `comprobar`, QUE ES EL QUE LOS JUNTA TODOS. Se
    // pintan TODOS antes de decidir si se sigue: un compilador que para en el
    // primero obliga a compilar diez veces para ver diez errores.
    let revisado = bmo_inti_front::comprobar(texto);
    let mut pintados = String::new();
    let mut hay_error = false;
    for a in &revisado.avisos {
        pintados.push_str(&a.pintar(nombre));
        if a.codigo.0.starts_with('E') {
            hay_error = true;
        }
    }
    if hay_error {
        return Err(Fallo::Avisos(pintados));
    }

    let arbol = bmo_inti_front::armar(texto);
    let modulos = bmo_inti_front::tablas::Modulos::cargar(raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(raices),
    );

    // -- El descenso y los bytes -------------------------------------------
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, raices, &modulos);
    let ir = bmo_inti_front::ir::bajar_con(
        &arbol.valor,
        &modulos,
        &plano.valor,
        &metal,
        &bmo_inti_front::necesidades::Necesidades::cargar(raices),
    )
    .valor;
    let emitido = emitir(&ir);

    // ** LO QUE NO LLEGO A UN BYTE es un NO, no un aviso: un binario al que le
    // falta algo no hace lo que dice su fuente (ver `main.rs`, 2026-08-23).
    if !emitido.sin_emitir.is_empty() {
        return Err(Fallo::SinEmitir(emitido.sin_emitir.clone()));
    }

    // -- LO QUE EL BINARIO VA A DECIR DE SI MISMO: sale de `arbol` y de
    // `revisado`, los que YA se calcularon. Calcularlo por otro camino seria
    // describir un modulo distinto del que se acaba de emitir.
    let manifiesto = bmo_inti_front::manifiesto::de(&arbol.valor, &revisado.valor, nombre).a_toml();

    // -- EL GATE, y va antes de escribir: `empaquetar` llama a `bmo-verify`.
    let bytes = empaquetar(&emitido, Some(&manifiesto)).map_err(Fallo::Gate)?;
    Ok(Compilado { bytes, emitido, parte, eventos: eventos.len() })
}

/// Lo mismo, leyendo el fichero. Es lo que usa el metro.
pub fn compilar_fichero(ruta: &Path) -> Result<Vec<u8>, String> {
    let texto = std::fs::read_to_string(ruta).map_err(|e| format!("no puedo leer {}: {e}", ruta.display()))?;
    let raices = bmo_mods::Roots::find();
    compilar(&texto, &ruta.display().to_string(), &raices)
        .map(|c| c.bytes)
        .map_err(|f| f.to_string())
}
