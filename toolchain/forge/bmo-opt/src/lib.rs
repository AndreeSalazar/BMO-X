//! `bmo-opt` — Optimización genérica (1ª librería del pipeline).
//!
//! La "genérica, lo clásico optimiza": pasadas independientes de la máquina
//! que compensan la duplicación (que cada lenguaje reescriba su optimizador).
//! Es una **librería opcional a dial**: se empieza vacía (BEF directo, cero
//! optimización) y se sube el nivel cuando se quiera:
//!
//!   1. constant folding   (barato, gran retorno: `2+3` → `5`)
//!   2. dead-code elimination
//!   3. strength reduction (`x*8` → `x<<3`)
//!   4. register allocation lineal (el que más importa para loops COBOL)
//!
//! Resuelve el "loop tonto de COBOL": la aritmética pesada sí necesita este
//! eje (el cambio de modelo borra el tax del sistema, no la física del
//! cómputo — eso lo arregla la optimización).
//!
//! NO es un embudo: el frontend llama a `optimize(&mut ir)` si quiere. C/C++
//! pueden saltárselo para control total. Estado: esqueleto (nivel 0).

/// Nivel de optimización solicitado (el "dial").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptLevel {
    /// Nada: el IR pasa tal cual (transparente, predecible).
    None,
    /// Barato y de alto retorno: const-fold + DCE.
    Basic,
    /// + register allocation lineal + strength reduction.
    Full,
}

/// TODO(migración paso 5): `pub fn optimize(module: &mut IrModule, level: OptLevel)`.
/// Nivel 0 hoy: no-op, para que el pipeline compile contra este crate.
pub fn optimize_noop(_level: OptLevel) {}
