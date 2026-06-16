//! `nexo` — Runtime BSF loader para Ring 3 en FastOS
//!
//! Este crate carga y valida shaders BSF (BareX Shader Format).
//! No compila shaders — eso lo hace `nexo-sh-tool` en build time.
//!
//! Uso en Ring 3:
//! ```no_run
//! let bsf = nexo::shader::load(bsf_bytes)?;
//! let bytecode = bsf.spirv_bytes();
//! ```

#![no_std]

extern crate alloc;

pub mod shader;
pub mod abi;
