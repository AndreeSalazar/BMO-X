//! BMO bytecode emitter (stub for v1.8.0).
//!
//! The real implementation is the codegen module which produces
//! BMO bytecode directly. This is a placeholder for legacy
//! `bmoasm::emit` calls.

#![allow(dead_code)]

/// 64-bit x86 register operand.
pub struct Reg64(pub u8);

/// BMO bytecode emitter.
pub struct Emitter {
    // Placeholder fields
}

impl Emitter {
    pub fn new() -> Self { Self {} }
    pub fn emit(&mut self) -> &[u8] { &[] }
}

impl Default for Emitter {
    fn default() -> Self { Self::new() }
}
