//! BMO ABI type system — function signatures, field descriptors, calling conventions.
//!
//! Companion to `runtime::types` (TypeMeta/TypeRegistry). This module defines the
//! **typed** metadata that TypeRegistry can optionally carry: field layouts,
//! function parameter lists, and calling convention descriptors.
//!
//! # Relationship with runtime::types
//!
//! ```text
//! runtime::types::TypeMeta          ←  32-byte fixed header (BEF-compatible)
//!   name_hash, size, align, kind, field_count
//!
//! types::TypeField                  ←  per-field descriptor (name + type + offset)
//! types::FunctionSignature          ←  params + return type + calling convention
//! types::CallingConvention          ←  register assignment, stack rules
//! types::TypeKind                   ←  extends runtime type kind with signature info
//! ```
//!
//! Language frontends emit these into a BEF `.type_map` section (SectionKind::TypeMap).
//! The kernel's TypeRegistry loads them at boot and uses them for:
//! - Reflection (field access by name)
//! - LangBridge marshaling (typed arg conversion)
//! - VTable signature validation
//! - Debug symbol resolution

pub mod convention;
pub mod field;
pub mod signature;

pub use convention::*;
pub use field::*;
pub use signature::*;
