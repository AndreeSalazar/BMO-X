//! `lang_bridge` — puente para CUALQUIER lenguaje (presente o futuro).
//!
//! Cada lenguaje que quiera correr nativamente en FastOS registra un
//! [`LangDescriptor`] con sus particularidades de calling convention,
//! mangling, exception model, layout de strings, etc. El loader BEF lo
//! consulta automáticamente.
//!
//! ## Lenguajes contemplados (extensible)
//!
//! - **Rust** (`LANG_RUST`)            — host nativo del kernel
//! - **C / C++** (`LANG_C`, `LANG_CPP`) — vía thunks `compat::`
//! - **Zig** (`LANG_ZIG`)              — semántica casi idéntica a Rust
//! - **Swift** (`LANG_SWIFT`)          — Swift Calling Convention → BMO
//! - **Kotlin/Java** (`LANG_JVM`)      — necesita GC iface
//! - **C# / .NET** (`LANG_CLR`)        — necesita GC iface
//! - **Python** (`LANG_PYTHON`)        — necesita GC iface + boxing
//! - **JavaScript** (`LANG_JS`)        — necesita GC iface + tagged ptrs
//! - **Go** (`LANG_GO`)                — calling conv en stack, GC propio
//! - **OCaml** (`LANG_OCAML`)          — values 63-bit + GC
//! - **Lua** (`LANG_LUA`)              — values tagged + GC
//! - **Haskell** (`LANG_HASKELL`)      — lazy thunks, STG
//! - **Erlang/Elixir** (`LANG_BEAM`)   — actor model
//! - **Future** (`LANG_FUTURE_*`)      — slot reservado para nuevos lenguajes

#![allow(dead_code)]

pub mod descriptor;
pub mod registry;
pub mod ids;
pub mod features;

pub use descriptor::LangDescriptor;
pub use registry::LangRegistry;
