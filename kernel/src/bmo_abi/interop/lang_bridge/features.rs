//! Bitflags de features que un lenguaje soporta. Dirige decisiones del loader.

use crate::bmo_abi::primitives::bx_u64;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LangFeatures: bx_u64 {
        /// El lenguaje tiene un GC tracing/conservativo (necesita `gc_iface`).
        const NEEDS_GC          = 1 << 0;
        /// Excepciones unwind-style (C++/Java/Python).
        const HAS_EXCEPTIONS    = 1 << 1;
        /// Closures de primera clase (`closure::`).
        const HAS_CLOSURES      = 1 << 2;
        /// Dispatch dinámico via vtables (`vtable::`).
        const HAS_DYNAMIC_DISPATCH = 1 << 3;
        /// Genéricos / templates (mono o erased).
        const HAS_GENERICS      = 1 << 4;
        /// Reflection runtime.
        const HAS_REFLECTION    = 1 << 5;
        /// Async/await coroutines.
        const HAS_ASYNC         = 1 << 6;
        /// Macros higiénicas (Rust/Scheme/Racket).
        const HAS_MACROS        = 1 << 7;
        /// Lazy evaluation por defecto (Haskell).
        const LAZY_DEFAULT      = 1 << 8;
        /// Tagged pointers / NaN-boxing (JS/Lua).
        const TAGGED_VALUES     = 1 << 9;
        /// Strings inmutables interned (Java, Python, Erlang).
        const INTERNED_STRINGS  = 1 << 10;
        /// Memoria manual (`malloc`/`free` o `Box`/`Drop`).
        const MANUAL_MEMORY     = 1 << 11;
        /// Borrow checker / ownership (Rust).
        const OWNERSHIP         = 1 << 12;
        /// Effect handlers / algebraic effects (OCaml 5, Koka).
        const EFFECTS           = 1 << 13;
    }
}
