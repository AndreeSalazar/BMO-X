//! `barex::bmoasm` — **BMO simple**: lenguaje semántico-puro con sintaxis en
//! español que **emite bytes precisos** al CPU sin depender de compilador externo.
//!
//! ## Filosofía
//!
//! - **Self-hosted.** Vive en el kernel `no_std`, sin gcc/clang/cargo de Ring 3.
//! - **Sintaxis humana en español.** `def`, `si`, `mientras`, `retorna`, `reg`, `emit`.
//! - **Emisión directa.** El stmt `emit` escribe bytes literales al code stream.
//!   El resto del lenguaje genera bytes con un encoder x86-64 mínimo.
//! - **BMO ABI nativo.** Calling convention 7-GPR + 64 B align + RAX:RDX status.
//!   Sin Win64, sin SysV.
//! - **Cero dependencias.** Ni LLVM, ni MIR, ni `cc-rs`. Sólo Rust + bitops.
//!
//! ## Sintaxis (keywords base)
//!
//! ```bmo
//! def saluda(x: num) -> num {
//!     let total = x suma 10
//!     si total mayor 100 {
//!         retorna 0
//!     } sino {
//!         retorna total
//!     }
//! }
//!
//! def loop_test() {
//!     let i: num = 0
//!     mientras i menor 10 {
//!         emit 0x90 0x90          // dos NOPs literales
//!         let i = i suma 1
//!     }
//! }
//!
//! def acceso_directo() {
//!     reg rax = 42
//!     reg rbx = ptr 0x1000
//!     emit 0x0F 0x05              // syscall opcode literal
//! }
//! ```
//!
//! ## Estructura modular (Sesión 15) — sin monolitos
//!
//! ```
//!   bmoasm/
//!   ├── mod.rs       ← este archivo
//!   ├── _README.md   ← gramática y referencia rápida
//!   ├── lexer/       ← stream → Token (keywords español)
//!   ├── parser/      ← Token → AST (Node, Stmt, Expr)
//!   ├── sema/        ← análisis semántico (scopes, tipos básicos)
//!   ├── emit/        ← AST → bytes x86-64 (encoder mínimo)
//!   └── runtime/     ← helpers runtime (aloc/libre vía BMO ABI memory)
//! ```

#![allow(dead_code)]

pub mod lexer;
pub mod parser;
pub mod sema;
pub mod emit;
pub mod runtime;
pub mod builtin;
pub mod sample;
pub mod traductor;

// ─── Re-exports ──────────────────────────────────────────────────────
pub use lexer::{Token, TokenKind, Scanner};
pub use parser::{Ast, Stmt, Expr, Parser};
pub use sema::{Sema, SemaError};
pub use emit::{Emitter, EmitError, Reg64};
pub use builtin::{IntrinsicId, CpuFlag, emit_intrinsic, bytes_for};
pub use traductor::Traductor;

/// Versión del lenguaje BMO simple.
pub const BMOASM_VERSION: (u8, u8, u8) = (0, 1, 0);

/// Magic identificador del bytecode generado (puede ir en `SectionKind::Code`).
pub const BMOASM_MAGIC: u32 = u32::from_le_bytes(*b"BMOA");
