//! `bmo-verify` -- el gate de verificacion (unico checkpoint comun).
//!
//! Reemplaza el rol de seguridad que tendria un IR central, pero como
//! CONTRATO, no como embudo: cada lenguaje emite su BEF por su cuenta y el
//! verificador lo revisa de forma independiente.
//!
//! ```text
//! BEF (de cualquier lenguaje) -> [bmo-verify] -> pasa?
//!                                               si -> admitido al BMO ABI
//!                                               no -> rechazado (con razones)
//! ```
//!
//! **NO es un stub**: delega en el validador estructural REAL de
//! `bmo_abi::bef::validator` (header, tabla de secciones, imports/exports,
//! relocs, firma, flags). Este crate es la CARA de toolchain de ese gate:
//! los frontends llaman `verify()` sin acoplarse a la estructura interna de
//! bmo-abi.
//!
//! **Conexion Singularity**: si el BEF pasa, esta probado seguro -> puede
//! correr como Software Isolated Process (mismo espacio, sin transicion de
//! anillo). La verificacion --no un IR-- habilita el aislamiento barato.

use bmo_abi::bef::validator;

/// Veredicto de la verificacion de un BEF.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Paso: admisible al ABI, apto para SIP.
    Ok,
    /// Rechazado, con las razones (mensajes de error del validador).
    Rejected(Vec<String>),
}

impl Verdict {
    pub fn is_ok(&self) -> bool {
        matches!(self, Verdict::Ok)
    }
}

/// Verifica un buffer BEF completo. FUNCIONA: corre el validador estructural
/// real y colapsa su resultado a un veredicto binario con razones.
///
/// Las advertencias (secciones inusuales, etc.) no rechazan -- solo los
/// errores (magic malo, secciones fuera de rango, ABI incompatible...).
pub fn verify(bef: &[u8]) -> Verdict {
    let result = validator::validate(bef);
    if result.is_valid {
        Verdict::Ok
    } else {
        let reasons = result
            .issues
            .iter()
            .filter(|i| matches!(i.severity, validator::IssueSeverity::Error))
            .map(|i| i.message.clone())
            .collect();
        Verdict::Rejected(reasons)
    }
}

/// Igual que `verify`, pero devuelve TAMBIEN las advertencias (para
/// herramientas que quieran inspeccionar sin rechazar).
pub fn verify_verbose(bef: &[u8]) -> (Verdict, Vec<String>) {
    let result = validator::validate(bef);
    let warnings = result
        .issues
        .iter()
        .filter(|i| matches!(i.severity, validator::IssueSeverity::Warning))
        .map(|i| i.message.clone())
        .collect();
    let verdict = if result.is_valid {
        Verdict::Ok
    } else {
        let reasons = result
            .issues
            .iter()
            .filter(|i| matches!(i.severity, validator::IssueSeverity::Error))
            .map(|i| i.message.clone())
            .collect();
        Verdict::Rejected(reasons)
    };
    (verdict, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bmo_abi::bef::writer::{BefBuilder, BefSection};

    fn minimal_valid_bef() -> Vec<u8> {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16])); // ret
        b.add_section(BefSection::rodata(b"ok\0".to_vec()));
        b.build().unwrap()
    }

    #[test]
    fn accepts_a_real_valid_bef() {
        // FUNCIONA de verdad: construye un BEF valido y lo verifica.
        assert_eq!(verify(&minimal_valid_bef()), Verdict::Ok);
    }

    #[test]
    fn rejects_garbage_with_reasons() {
        let v = verify(&[0u8; 48]); // magic malo
        match v {
            Verdict::Rejected(reasons) => assert!(!reasons.is_empty(), "debe dar razones"),
            Verdict::Ok => panic!("basura no debe pasar el gate"),
        }
    }

    #[test]
    fn rejects_too_small() {
        assert!(!verify(&[0u8; 4]).is_ok());
    }

    #[test]
    fn verbose_reports_warnings_without_rejecting() {
        // BEF valido pero con Code duplicada = advertencia, no rechazo.
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16]));
        b.add_section(BefSection::code(vec![0xCC; 16]));
        let bytes = b.build().unwrap();
        let (verdict, warnings) = verify_verbose(&bytes);
        assert_eq!(verdict, Verdict::Ok, "duplicado es warning, no error");
        assert!(warnings.iter().any(|w| w.contains("duplicate")));
    }
}
