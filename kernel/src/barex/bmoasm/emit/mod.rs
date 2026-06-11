//! Emisor multi-arquitectura para BMO Simple.
//! Agrupa y modulariza los codegen backends por carpetas de arquitectura.

pub mod x86_64;
pub mod aarch64;
pub mod riscv;

// Re-exportar la arquitectura activa en tiempo de compilación para el OS.
// Dado que FastOS corre actualmente sobre x86-64, este es el backend activo por defecto.
pub use x86_64::reg::{Reg64, BMO_ARG_REGS};
pub use x86_64::encoder::{Emitter, EmitError};
