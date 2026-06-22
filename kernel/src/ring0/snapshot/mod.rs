//! `ring0::snapshot` — Marcas de bajo nivel para TimeBack.
//!
//! v1.8.8: stubs. Aquí viven los punteros y marcas que TimeBack
//! usa para encontrar las estructuras a snapshot/rollback.
//!
//! ## Componentes
//!
//! - `memory_mark`: marca páginas que están "sucias" (cambiaron).
//! - `process_mark`: marca procesos vivos en un instante.

#![allow(dead_code)]

pub mod memory_mark;
pub mod process_mark;

pub fn init() {
    memory_mark::init();
    process_mark::init();
}
