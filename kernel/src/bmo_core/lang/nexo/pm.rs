//! ÑEXO Package Manager — Gestor de paquetes nativo.
//!
//! Formato de manifiesto: `nexo.toml` (TOML simplificado).
//! Registry local, resolución de dependencias, build system.

#![allow(dead_code)]

pub mod manifest;
pub mod registry;
pub mod resolver;
pub mod build;

pub fn init() {
    crate::bmo_core::diag::info("nexo_pm", "Package manager initialized");
}
