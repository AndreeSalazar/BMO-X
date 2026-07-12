//! `defense::bytedefender` — Orquestador principal.

#![allow(dead_code)]

pub struct ByteDefender {
    /// ¿Está habilitado el pre-execution check?
    pub pre_exec: bool,
    /// ¿Está habilitado el runtime guard?
    pub runtime_guard: bool,
    /// # de apps inspeccionadas (cum).
    pub inspected: u64,
    /// # de apps rechazadas (cum).
    pub rejected: u64,
    /// # de apps en cuarentena (cum).
    pub quarantined: u64,
}

impl ByteDefender {
    pub const fn new() -> Self {
        Self {
            pre_exec: true,
            runtime_guard: true,
            inspected: 0,
            rejected: 0,
            quarantined: 0,
        }
    }
}
