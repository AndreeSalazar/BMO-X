//! `reflect` — reflection runtime sobre cualquier BEF cargado.
//!
//! Reemplaza:
//!   - Java `java.lang.reflect.*`
//!   - C# `System.Reflection`
//!   - Go `reflect`
//!   - Python `inspect`, `dir()`, `getattr()`
//!
//! Una sola API en C/Rust/Swift accede a metadatos de cualquier módulo BEF
//! sin importar el lenguaje fuente. Los datos vienen de:
//!   - `SectionKind::TypeMap`     (descriptores, ver `type_system::registry`)
//!   - `SectionKind::Symbols`     (nombres simbólicos, `bef::symbols`)
//!   - `SectionKind::Manifest`    (capabilities, autor, versión)

#![allow(dead_code)]

pub mod mirror;
pub mod query_api;

pub use query_api::ReflectQuery;
