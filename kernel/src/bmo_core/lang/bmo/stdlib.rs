//! BMO Standard Library — Librería estándar nativa de BMO/FastOS.
//!
//! Todos los módulos son implementaciones nativas, sin dependencias
//! de Windows/Linux/Mac. Todo se construye desde cero para BMO/FastOS.
//!
//! ## Módulos
//!
//! - `io` — E/S serial y framebuffer
//! - `mem` — Gestión de memoria
//! - `str` — Operaciones con strings
//! - `math` — Aritmética
//! - `fs` — Sistema de archivos
//! - `proc` — Gestión de procesos
//! - `time` — Reloj y temporización
//! - `gfx` — Primitivas gráficas
//! - `sys` — Llamadas al sistema BMO
//! - `net` — Operaciones de red
//! - `env` — Entorno y argumentos
//! - `path` — Manipulación de rutas
//! - `collections` — Colecciones básicas

#![allow(dead_code)]

pub mod io;
pub mod mem;
pub mod str;
pub mod math;
pub mod fs;
pub mod proc;
pub mod time;
pub mod gfx;
pub mod sys;
pub mod net;
pub mod env;
pub mod path;
pub mod collections;

/// Standard library version.
pub const STDLIB_VERSION: (u8, u8, u8) = (0, 1, 0);
