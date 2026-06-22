//! BMO Package Manager — Gestor de paquetes nativo.
//!
//! Formato de manifiesto: `bmo.toml` (TOML simplificado).
//! Registry local, resolución de dependencias, build system.

#![allow(dead_code)]

pub mod manifest;
pub mod registry;
pub mod resolver;
pub mod build;

pub fn init() {
    crate::bmo_core::diag::info("bmo_pm", "Package manager initialized");
}
