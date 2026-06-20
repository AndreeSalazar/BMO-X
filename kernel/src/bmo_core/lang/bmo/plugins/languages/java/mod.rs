//! Java language — essential subset (Dalvik-style minimal JVM).
//!
//! Supports: classes (single inheritance), interfaces, methods,
//! fields, constructors, virtual dispatch via vtable, basic
//! try/catch, primitive types, arrays, strings, `new`.
//!
//! Does NOT support: generics, reflection, annotations runtime,
//! lambdas, streams, modules, sealed classes, records.

#![allow(dead_code)]

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod translator;
pub mod vtable;
pub mod exceptions;
pub mod plugin;
