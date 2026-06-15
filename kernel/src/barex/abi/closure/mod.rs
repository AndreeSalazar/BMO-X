//! `closure` — closures de primera clase del BMO ABI.
//!
//! C ABI **no tiene** closures: pasar contexto requiere un `void* user_data`
//! manual en cada callback. BMO ABI los tiene como ciudadanos de primera.
//!
//! Layout idéntico a `Box<dyn FnMut>` de Rust pero `repr(C)` y compatible
//! con cualquier lenguaje con cierres (Swift, Kotlin, JS, Python, Lua...).

#![allow(dead_code)]

pub mod boxed;
pub mod env;
pub mod signature;

