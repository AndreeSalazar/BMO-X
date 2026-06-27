//! `cabina::filter` — Filtros inteligentes para el log de eventos.
//!
//! El log puede tener 256+ eventos. Los filtros permiten que el
//! overlay (y futuros apps Ring 3) vean solo lo relevante.
//!
//! ## Filtros disponibles
//!
//! - **Por severidad**: solo PANIC, FAULT, etc.
//! - **Por módulo**: solo eventos de "fs", "lang", "kbc", ...
//! - **Por texto**: substring search en el msg
//! - **Por rango de tiempo**: solo los últimos N segundos

#![allow(dead_code)]

use crate::cabina::event::{Event, Severity};

/// Un filtro sobre eventos.
#[derive(Clone, Debug, Default)]
pub struct EventFilter {
    /// Severidad mínima (None = todas).
    pub min_severity: Option<Severity>,
    /// Solo estos módulos (vacío = todos).
    pub modules: alloc::vec::Vec<alloc::string::String>,
    /// Solo mensajes que contengan este substring (None = todos).
    pub msg_contains: Option<alloc::string::String>,
    /// Solo eventos con seq >= this.
    pub min_seq: Option<u64>,
}

impl EventFilter {
    pub const fn new() -> Self {
        Self {
            min_severity: None,
            modules: alloc::vec::Vec::new(),
            msg_contains: None,
            min_seq: None,
        }
    }

    /// `true` si el evento pasa el filtro.
    pub fn matches(&self, ev: &Event) -> bool {
        if let Some(min) = self.min_severity {
            if ev.severity < min { return false; }
        }
        if !self.modules.is_empty() {
            if !self.modules.iter().any(|m| m == &ev.module) { return false; }
        }
        if let Some(ref s) = self.msg_contains {
            if !ev.msg.contains(s.as_str()) { return false; }
        }
        if let Some(min_seq) = self.min_seq {
            if ev.seq < min_seq { return false; }
        }
        true
    }

    /// Solo PANIC y FAULT.
    pub fn only_critical() -> Self {
        Self { min_severity: Some(Severity::Fault), ..Self::new() }
    }

    /// Solo un módulo específico.
    pub fn only_module(name: &str) -> Self {
        Self { modules: alloc::vec![alloc::string::String::from(name)], ..Self::new() }
    }

    /// Solo mensajes que contengan el substring.
    pub fn search(s: &str) -> Self {
        Self { msg_contains: Some(alloc::string::String::from(s)), ..Self::new() }
    }
}

/// Aplica un filtro a una lista de eventos.
pub fn apply(events: &[Event], filter: &EventFilter) -> alloc::vec::Vec<Event> {
    events.iter().filter(|e| filter.matches(e)).cloned().collect()
}
