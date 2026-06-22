//! `ring0::security::exec_hook` — Hook pre-ejecución de Ring 3.
//!
//! Se llama justo antes de hacer `iretq` o `sysret` hacia Ring 3.
//! Verifica capabilities y la integridad del proceso.

#![allow(dead_code)]

/// Resultado del hook de ejecución.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecVerdict {
    Allow,
    Deny,
}

/// Llamado antes de saltar a Ring 3. v1.8.8: stub.
pub fn check(_cr3: u64, _entry: u64) -> ExecVerdict {
    ExecVerdict::Allow
}

pub fn init() {}
