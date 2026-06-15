//! `vtable` — despacho dinámico genérico del BMO ABI.
//!
//! Reemplaza:
//!   - C++ vtable (Itanium / MSVC ABI)
//!   - COM `IUnknown::QueryInterface`
//!   - Rust trait objects (`dyn Trait` fat pointer)
//!   - Java/Kotlin/Swift interface dispatch
//!   - Go iface (itab)
//!   - Python `tp_methods`, `__getattr__`
//!
//! Una sola convención: `BmoVTable`. Cualquier lenguaje con polimorfismo
//! dinámico puede generar/consumir vtables BMO sin glue per-language.

#![allow(dead_code)]

pub mod table;
pub mod entry;
pub mod fat_ptr;
pub mod query;

pub use table::BmoVTable;
