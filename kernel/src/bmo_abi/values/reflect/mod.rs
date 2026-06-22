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

use crate::bmo_abi::primitives::{bx_usize, bx_u32};
use crate::bmo_abi::values::string::BmoStr;

pub mod mirror;
pub mod query_api;

pub use query_api::ReflectQuery;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Primitive = 0,
    Struct    = 1,
    Enum      = 2,
    Function  = 3,
    Pointer   = 4,
    Array     = 5,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TypeDescriptor<'a> {
    pub name: BmoStr<'a>,
    pub size: bx_usize,
    pub align: bx_usize,
    pub kind: TypeKind,
    pub _pad: bx_u32,
}

impl<'a> TypeDescriptor<'a> {
    pub const fn new(name: BmoStr<'a>, size: bx_usize, align: bx_usize, kind: TypeKind) -> Self {
        Self { name, size, align, kind, _pad: 0 }
    }
}
