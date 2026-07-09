//! `option` — BmoOption<T>, un Option<T> FFI-safe para el BMO ABI.
//!
//! A diferencia de `core::option::Option<T>`, el layout está garantizado
//! por `#[repr(C)]` con un discriminante explícito `bx_u64`. Esto evita
//! las optimizaciones de nicho de Rust (que son geniales para Rust, pero
//! letales para FFI).

use crate::bmo_abi::primitives::bx_u64;

/// FFI-safe optional value.
///
/// # Layout (16 bytes)
/// ```text
/// [0..7]  value: T       (zero-initialized when none)
/// [8..15] has_value: u64  (0 = None, 1 = Some)
/// ```
///
/// Pairs with `BmoResult<T>` for fallible returns that carry no error detail.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoOption<T: Copy> {
    value: T,
    has_value: bx_u64,
}

impl<T: Copy> BmoOption<T> {
    pub const NONE: Self = Self {
        value: unsafe { core::mem::zeroed() },
        has_value: 0,
    };

    pub const fn some(v: T) -> Self {
        Self { value: v, has_value: 1 }
    }

    pub fn is_some(&self) -> bool {
        self.has_value != 0
    }

    pub fn is_none(&self) -> bool {
        self.has_value == 0
    }

    pub fn unwrap(self) -> T {
        assert!(self.has_value != 0, "BmoOption::unwrap on None");
        self.value
    }

    pub fn unwrap_or(self, default: T) -> T {
        if self.has_value != 0 { self.value } else { default }
    }

    pub fn map<U: Copy>(self, f: impl FnOnce(T) -> U) -> BmoOption<U> {
        if self.has_value != 0 {
            BmoOption::some(f(self.value))
        } else {
            BmoOption::NONE
        }
    }
}

impl<T: Copy + PartialEq> PartialEq for BmoOption<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self.has_value != 0, other.has_value != 0) {
            (false, false) => true,
            (true, true) => self.value == other.value,
            _ => false,
        }
    }
}

// ─── Conversion helpers ─────────────────────────────────────────────

impl<T: Copy> From<Option<T>> for BmoOption<T> {
    fn from(o: Option<T>) -> Self {
        match o {
            Some(v) => BmoOption::some(v),
            None => BmoOption::NONE,
        }
    }
}

impl<T: Copy> From<BmoOption<T>> for Option<T> {
    fn from(b: BmoOption<T>) -> Self {
        if b.is_some() { Some(b.value) } else { None }
    }
}
