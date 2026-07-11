//! Ring 3 — Orquestador de modo usuario.
//!
//! `kernel::ring3` orquesta la transición y gestión de Ring 3.
//! Las implementaciones pesadas viven en `crates_Personal/`.
//!
//! ## Módulos
//!
//! - `transition` — iretq primitivo (la única instrucción CPU)
//! - `entry`      — preparación de tasks, consulta de privilegio
//! - `loader`     — carga de módulos Ring 3 (bmo_core, etc.)

pub mod transition;
pub mod entry;
pub mod loader;
pub mod gateway;
