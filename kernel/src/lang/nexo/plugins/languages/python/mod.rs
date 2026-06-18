//! Python language — essential subset (MicroPython-style).
//!
//! Supports: def, class, if/elif/else, while, for-in, try/except, with,
//! import, literals (int, float, str, bool, None, list, dict, tuple),
//! function calls, list/dict comprehensions (basic), lambdas (no capture).
//!
//! Does NOT support: generators (yield), async/await, decorators with
//! arguments, metaclass, descriptors, complex slicing.

#![allow(dead_code)]

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod translator;
pub mod builtins;
pub mod plugin;
