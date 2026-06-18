//! `interop` — cómo se HABLA con otros lenguajes y otros ABIs.
//!
//! - [`lang_bridge`]  — registro de lenguajes (Rust, C++, Java, Swift, etc).
//! - [`marshal`]      — conversiones Lang ↔ BMO ↔ Lang.
//! - [`compat`]       — thunks Win64 / SysV → BMO ABI para FFI con código C heredado.

pub mod lang_bridge;
pub mod marshal;
pub mod compat;
