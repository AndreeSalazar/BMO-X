//! ByteDefender: BMO pre-execution security scanner and runtime capability guard.
//!
//! v1.8.8: analyzes and protects:
//!
//! - **Pre-execution**: BEF before loading (headers, relocations, imports,
//!   capabilities, W/X sections).
//! - **Runtime guard**: monitors dangerous Ring 3 syscalls.
//!
//! ## Golden rule
//!
//! - ByteDefender **does not paint UI**. It only analyzes and reports.
//! - Cabina displays ByteDefender reports.
//! - TimeBack can create checkpoints before executing defended apps.

#![no_std]

extern crate alloc;

pub mod bytedefender;
pub mod policy;
pub mod scanner;
pub mod verifier;
pub mod capability;
pub mod report;
pub mod quarantine;

#[cfg(test)]
mod tests;

pub use bytedefender::*;
pub use capability::*;
pub use report::*;
pub use scanner::*;
pub use verifier::*;
