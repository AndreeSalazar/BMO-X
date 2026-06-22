//! `lang::runtimes` — Todos los runtimes disponibles.
//!
//! Un runtime es código que se linkea con un binario compilado para
//! darle soporte del lenguaje (memcpy, malloc, GC, RTTI, etc).
//!
//! ## Runtimes
//!
//! - `c_min` — Runtime C mínimo (memcpy, strlen, syscall wrappers).
//!            Es el único runtime activo en v1.8.8.

#![allow(dead_code)]

pub mod c_min;
