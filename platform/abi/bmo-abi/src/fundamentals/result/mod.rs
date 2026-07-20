//! `result` — BmoResult<T, E>, un Result<T, E> FFI-safe para el BMO ABI.
//!
//! Layout fijo `#[repr(C)]` con discriminante explícito. Cada variante
//! lleva su payload completo; la variante no usada se zero-inicializa.
//!
//! Para errores sin detalle, `BmoResult<T, ()>` equivale a
//! `BmoOption<T>` pero con semántica de error.

use crate::bmo_abi::fundamentals::status::BmoStatus;
use crate::bmo_abi::primitives::bx_u64;

/// FFI-safe result type.
///
/// # Layout (32 bytes + payloads)
/// ```text
/// [0..N)     ok: T        (zeroed when err)
/// [N..N+M)   err: E       (zeroed when ok)
/// [N+M..]    tag: u64     (0 = Ok, 1 = Err)
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BmoResult<T: Copy, E: Copy> {
    ok: T,
    err: E,
    tag: bx_u64,
}

impl<T: Copy, E: Copy> BmoResult<T, E> {
    pub fn ok(v: T) -> Self {
        Self {
            ok: v,
            err: unsafe { core::mem::zeroed() },
            tag: 0,
        }
    }

    pub fn err(e: E) -> Self {
        Self {
            ok: unsafe { core::mem::zeroed() },
            err: e,
            tag: 1,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.tag == 0
    }
    pub fn is_err(&self) -> bool {
        self.tag != 0
    }

    pub fn unwrap(self) -> T {
        assert!(self.tag == 0, "BmoResult::unwrap on Err");
        self.ok
    }

    pub fn unwrap_err(self) -> E {
        assert!(self.tag != 0, "BmoResult::unwrap_err on Ok");
        self.err
    }

    pub fn ok_value(self) -> Option<T> {
        if self.tag == 0 {
            Some(self.ok)
        } else {
            None
        }
    }

    pub fn map<U: Copy>(self, f: impl FnOnce(T) -> U) -> BmoResult<U, E> {
        if self.tag == 0 {
            BmoResult::ok(f(self.ok))
        } else {
            BmoResult::err(self.err)
        }
    }

    pub fn map_err<F: Copy>(self, f: impl FnOnce(E) -> F) -> BmoResult<T, F> {
        if self.tag == 0 {
            BmoResult::ok(self.ok)
        } else {
            BmoResult::err(f(self.err))
        }
    }
}

impl<T: Copy, E: Copy + PartialEq> PartialEq for BmoResult<T, E>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (self.tag, other.tag) {
            (0, 0) => self.ok == other.ok,
            (1, 1) => self.err == other.err,
            _ => false,
        }
    }
}

// ─── Specialization: BmoResult<T, BmoStatus> ───────────────────────

impl<T: Copy> BmoResult<T, BmoStatus> {
    /// Convert to a `BmoStatus`, discarding the ok value.
    pub fn into_status(self) -> BmoStatus {
        if self.tag == 0 {
            BmoStatus::OK
        } else {
            self.err
        }
    }

    /// Convert from `BmoStatus`. Ok if status.is_ok(), using `Default` for T.
    pub fn from_status(s: BmoStatus, default_ok: T) -> Self {
        if s.is_ok() {
            BmoResult::ok(default_ok)
        } else {
            BmoResult::err(s)
        }
    }
}

// ─── Conversion ─────────────────────────────────────────────────────

impl<T: Copy, E: Copy> From<Result<T, E>> for BmoResult<T, E> {
    fn from(r: Result<T, E>) -> Self {
        match r {
            Ok(v) => BmoResult::ok(v),
            Err(e) => BmoResult::err(e),
        }
    }
}

impl<T: Copy, E: Copy> From<BmoResult<T, E>> for Result<T, E> {
    fn from(b: BmoResult<T, E>) -> Self {
        if b.is_ok() {
            Ok(b.ok)
        } else {
            Err(b.err)
        }
    }
}
