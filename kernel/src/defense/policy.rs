//! `defense::policy` — Reglas de seguridad.

#![allow(dead_code)]

/// Acción que toma ByteDefender cuando algo se sale de la policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyAction {
    /// Permitir la syscall.
    Allow,
    /// Loguear y permitir (vigilancia).
    Warn,
    /// Bloquear la syscall.
    Deny,
}

/// Conjunto de reglas (stub).
pub struct Policy {
    pub allow_all_syscalls: bool,
    pub allow_wx_sections: bool,
    pub allow_unsigned_bef: bool,
}

impl Policy {
    pub const fn default_strict() -> Self {
        Self {
            allow_all_syscalls: false,
            allow_wx_sections: false,
            allow_unsigned_bef: false,
        }
    }
}

pub fn init() {
    // v1.8.8: sin storage persistente. La policy es compile-time.
}

/// Verifica si el sistema tiene todas las capabilities requeridas.
pub fn has_all_caps(_req: &[Capability]) -> bool {
    // v1.8.8: stub. Todas las capabilities están disponibles.
    true
}

/// Hook de runtime para una syscall específica.
pub fn check_syscall(_nr: u16) -> PolicyAction {
    // v1.8.8: stub. Permitir todo, loggear en v1.9.
    PolicyAction::Allow
}

// Forward decl para que compile.
use super::capability::Capability;
