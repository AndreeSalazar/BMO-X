//! Backend x86-64 para el emisor de BMO Simple.

pub mod reg;
pub mod encoder;

pub use reg::{Reg64, BMO_ARG_REGS};
pub use encoder::{Emitter, EmitError};
