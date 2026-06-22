//! `defense::report` — Reporte de seguridad.

extern crate alloc;

#![allow(dead_code)]

use alloc::string::String;

/// Veredicto final sobre una app/BEF.
#[derive(Clone, Debug)]
pub enum Verdict {
    /// Permitir la ejecución.
    Allow,
    /// Bloquear y rechazar.
    Reject(String),
    /// Bloquear y poner en cuarentena (permite análisis posterior).
    Quarantine(String),
}

impl Verdict {
    pub fn is_allow(&self) -> bool { matches!(self, Verdict::Allow) }
    pub fn is_reject(&self) -> bool { matches!(self, Verdict::Reject(_)) }
    pub fn is_quarantine(&self) -> bool { matches!(self, Verdict::Quarantine(_)) }
}

/// Reporte de seguridad generado al inspeccionar un BEF.
#[derive(Clone, Debug)]
pub struct SecurityReport {
    pub name: String,
    pub verdict: Verdict,
    pub capabilities_required: u32,
    pub has_wx: bool,
    pub section_count: u16,
    pub hash: u64,
}

impl SecurityReport {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            verdict: Verdict::Allow,
            capabilities_required: 0,
            has_wx: false,
            section_count: 0,
            hash: 0,
        }
    }
}
