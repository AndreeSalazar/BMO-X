//! `bmo-verify` — el gate de verificación (único checkpoint común).
//!
//! Reemplaza el rol de seguridad que tendría un IR central, pero como
//! CONTRATO, no como embudo: cada lenguaje emite su BEF por su cuenta y el
//! verificador lo revisa de forma independiente.
//!
//! ```text
//! BEF (de cualquier lenguaje) → [bmo-verify] → ¿pasa?
//!                                               sí → admitido al BMO ABI
//!                                               no → rechazado
//! ```
//!
//! Verifica: solo operaciones de capability válidas, memory-safety, respeto
//! al ABI congelado, firma/hash. Crece desde el esqueleto ya presente en
//! `platform/abi/bmo-abi/src/bef/{validator,signing,blake3}.rs`.
//!
//! **Conexión Singularity**: si el BEF pasa, está probado seguro → puede
//! correr como Software Isolated Process (mismo espacio, sin transición de
//! anillo). La verificación —no un IR— habilita el aislamiento barato.
//!
//! Estado: esqueleto. Paso de migración 6.

/// Veredicto de la verificación de un BEF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Pasó: admisible al ABI, apto para SIP.
    Ok,
    /// Rechazado, con la razón.
    Rejected,
}

/// TODO(migración paso 6): `pub fn verify(bef: &[u8]) -> Verdict` reusando
/// `bmo-abi::bef::validator` + firma. Por ahora placeholder para que el
/// pipeline compile contra el gate.
pub fn verify_stub() -> Verdict {
    Verdict::Ok
}
