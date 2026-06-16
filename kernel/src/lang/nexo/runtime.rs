//! ÑEXO Runtime — Capa de runtime para programas ÑEXO.
//!
//! Wrappear servicios del kernel BMO/FastOS en APIs de alto nivel
//! para programas ÑEXO. Todo nativo, sin dependencias externas.
//!
//! ## Módulos
//!
//! - `mem` — Gestión de memoria (pool allocator sobre bump)
//! - `proc` — Procesos y hilos (spawn, exit, wait)
//! - `io` — E/S serial y framebuffer
//! - `fs` — Sistema de archivos (lectura)
//! - `time` — Reloj y sleep
//! - `error` — Tipos de error

#![allow(dead_code)]

pub mod error;
pub mod mem;
pub mod proc;
pub mod io;
pub mod fs;
pub mod time;

/// Runtime version.
pub const RUNTIME_VERSION: (u8, u8, u8) = (0, 1, 0);

/// Initialize the ÑEXO runtime.
pub fn init() {
    crate::diag::info("nexo_rt", "Runtime ÑEXO inicializado");
}
