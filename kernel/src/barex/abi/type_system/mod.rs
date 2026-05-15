//! `type_system` — descriptores universales de tipos del BMO ABI.
//!
//! Reemplaza:
//!   - C `<typeinfo>` y `_Generic`
//!   - C++ RTTI (`type_info`, vtable type-id)
//!   - .NET `System.Type`, Java `java.lang.Class`, Go `reflect.Type`
//!   - Python `PyTypeObject`, Swift `Metadata`
//!
//! Cualquier lenguaje (presente o futuro) que quiera correr sobre FastOS
//! describe sus tipos en términos de [`TypeDescriptor`]. Una vez en BMO,
//! todos los lenguajes interoperan **sin marshalling extra**.
//!
//! ## Filosofía
//!
//! Un `TypeDescriptor` es:
//!   - **Auto-descriptivo:** size, align, kind, layout completo.
//!   - **FFI-estable:** `#[repr(C)]`, sin generics ocultos.
//!   - **Hashable:** dos lenguajes pueden compararlos por hash BLAKE3.
//!   - **Serializable:** se embebe en secciones BEF (`SectionKind::TypeMap`).
//!   - **Componible:** structs/enums/arrays se construyen recursivamente.

#![allow(dead_code)]

pub mod descriptor;
pub mod kind;
pub mod layout;
pub mod registry;
pub mod hash;

pub use descriptor::{TypeDescriptor, TypeId, FieldDescriptor, VariantDescriptor};
pub use kind::TypeKind;
pub use layout::TypeLayout;
pub use registry::TypeRegistry;
pub use hash::type_hash;

/// Versión del sistema de tipos BMO.
pub const TYPE_SYSTEM_VERSION: (u8, u8) = (1, 0);
