//! C++ language — essential subset.
//!
//! Supports: classes (struct-like), inheritance, member functions,
//! constructors, destructors, `this` pointer, virtual methods with
//! vtable, `new`/`delete`, public/private/protected.
//!
//! Does NOT support: templates, exceptions, operator overloading,
//! multiple inheritance, virtual inheritance, RTTI, namespaces.
//!
//! Lowered to BMO AST via struct + vtable pattern.

#![allow(dead_code)]

pub mod ast;
pub mod lexer;
pub mod translator;
pub mod plugin;
