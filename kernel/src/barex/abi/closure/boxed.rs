//! `BmoClosure` — closure boxed. 32 bytes, FFI-estable.
//!
//! Replicable layout para cualquier lenguaje:
//!   - Rust:   `Box<dyn FnMut(...)>` ↔ `BmoClosure`
//!   - C++:    `std::function<...>`  ↔ `BmoClosure`
//!   - Swift:  `(...) -> ...` capturing closure ↔ `BmoClosure`
//!   - JS:     `function() { ... }` con `[[Environment]]` ↔ `BmoClosure`

use crate::barex::abi::primitives::bx_u64;
use super::env::ClosureEnv;

/// Puntero a función "thunk" que recibe `env_ptr` como primer arg implícito.
pub type ClosureThunk = unsafe extern "C" fn(env_ptr: *mut u8 /*, args... */);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BmoClosure {
    /// Puntero al thunk (firma BMO ABI con `env_ptr` implícito en RDI).
    pub thunk: bx_u64,
    /// Entorno capturado (16 B header + payload por punter).
    pub env: ClosureEnv,
}

impl BmoClosure {
    pub const NULL: Self = Self { thunk: 0, env: ClosureEnv::EMPTY };

    #[inline(always)]
    pub const fn is_null(&self) -> bool { self.thunk == 0 }

    /// Crea desde puntero crudo a fn estática (sin entorno).
    #[inline(always)]
    pub const fn from_fn(f: bx_u64) -> Self {
        Self { thunk: f, env: ClosureEnv::EMPTY }
    }

    /// Llama el closure. Stub — la implementación real necesita conocer
    /// la signature; ver `signature::ClosureSig`.
    pub fn invoke_void(&self) {
        if self.thunk == 0 { return; }
        // SAFETY: el caller garantiza signature `(env_ptr) -> ()`.
        unsafe {
            let f: ClosureThunk = core::mem::transmute(self.thunk as usize);
            f(self.env.data_ptr as *mut u8);
        }
    }
}
