//! `ring0::security` — Hooks de seguridad a nivel Ring 0.
//!
//! v1.8.8: stubs. Aquí viven los hooks críticos que se ejecutan
//! en el camino caliente (exec, page fault, syscall).
//!
//! ## Componentes
//!
//! - `exec_hook`: se llama antes de saltar a Ring 3.
//! - `memory_policy`: aplica W^X y permisos en page tables.

#![allow(dead_code)]

pub mod exec_hook;
pub mod memory_policy;

pub fn init() {
    exec_hook::init();
    memory_policy::init();
}
